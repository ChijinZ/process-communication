// #![feature(nll)]
#![deny(warnings, missing_debug_implementations)]


extern crate tokio;
extern crate futures;
extern crate tokio_codec;
extern crate serde;
extern crate bincode;
extern crate bytes;

use tokio::net;
use tokio::prelude::*;
use bincode::{deserialize, serialize};
use tokio::io;
use tokio_codec::*;
use bytes::{BufMut, BytesMut};
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};
use futures::sync::mpsc;

// The number of bytes to represent data size.
const DATA_SIZE: usize = 4;

// This struct is to build a tokio framed to encode messages to bytes and decode bytes to messages.
// 'T' represents user-defined message type.
#[derive(Debug)]
struct MessageCodec<T> {
    name: String,
    phantom: PhantomData<T>,
}

impl<T> MessageCodec<T> {
    pub fn new(name: String) -> MessageCodec<T> {
        MessageCodec { name: name, phantom: PhantomData }
    }
}

// A u64 to Vec<u8> function to convert decimal to 256 hexadecimal.
pub fn number_to_four_vecu8(num: u64) -> Vec<u8> {
    assert!(num < (1 << 32));
    let mut result: Vec<u8> = vec![];
    let mut x = num;
    loop {
        if x / 256 > 0 {
            result.push((x % 256) as u8);
            x = x / 256;
        } else {
            result.push((x % 256) as u8);
            break;
        }
    }
    for _ in 0..(DATA_SIZE - result.len()) {
        result.push(0);
    }
    result.reverse();
    return result;
}

// A Vec<u8> to u64 function to convert 256 hexadecimal to decimal.
pub fn four_vecu8_to_number(vec: Vec<u8>) -> u64 {
    assert_eq!(vec.len(), DATA_SIZE);
    let num = vec[0] as u64 * 256 * 256 * 256 + vec[1] as u64 * 256 * 256
        + vec[2] as u64 * 256 + vec[3] as u64;
    return num;
}

impl<T> Encoder for MessageCodec<T> where T: serde::Serialize {
    type Item = (String, T);
    type Error = io::Error;
    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let mut data: Vec<u8> = vec![];
        data.append(&mut serialize(&item).unwrap());
        let mut encoder: Vec<u8> = number_to_four_vecu8(data.len() as u64);
        encoder.append(&mut data);
        dst.reserve(encoder.len());
        dst.put(encoder);
        Ok(())
    }
}

impl<T> Decoder for MessageCodec<T> where T: serde::de::DeserializeOwned {
    type Item = (String, T);
    type Error = io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < DATA_SIZE {
            Ok(None)
        } else {
            let mut vec: Vec<u8> = src.to_vec();
            let truth_data = vec.split_off(DATA_SIZE);
            let vec_length = four_vecu8_to_number(vec);
            if truth_data.len() == vec_length as usize {
                src.clear();
                Ok(Some(deserialize(&truth_data).unwrap()))
            } else {
                Ok(None)
            }
        }
    }
}

//T is user message type
//F is the closure of control logic
//U is to abstract tcp and uds
fn start_server<T, F, U>(listener_incoming: U,
                         register_logic: F,
                         user_tx: Arc<RwLock<Option<mpsc::Sender<(String, T)>>>>,
                         server_tx: Box<std::sync::mpsc::Sender<(String, T)>>,
                         server_name: String)
                         -> Box<Future<Item=(), Error=()> + Send + 'static>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone,
          F: FnMut(&str, u64) + Send + Sync + 'static + Clone,
          U: Stream + Send + Sync + 'static,
          <U as futures::Stream>::Item: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync,
          <U as futures::Stream>::Error: std::fmt::Debug + 'static + Send + Sync
{
    Box::new(
        listener_incoming
            .for_each(move |stream| {
                let register_logic = register_logic.clone();
                let server_name = server_name.clone();
                let server_tx = server_tx.clone();
                let user_tx = user_tx.clone();

                let (tx, rx): (mpsc::Sender<(String, T)>, mpsc::Receiver<(String, T)>) = mpsc::channel(0);
                let rx: Box<Stream<Item=(String, T), Error=io::Error> + Send> = Box::new(rx.map_err(|_| panic!()));

                // Split tcp_stream to sink and stream. Sink responses to send messages to this
                // client, stream responses to receive messages from this client.
                let (sink, stream) = MessageCodec::new(server_name).framed(stream).split();

                // Spawn a sender task.
                let send_to_client = rx.forward(sink).then(|result| {
                    if let Err(e) = result {
                        println!("failed to write to socket: {}", e)
                    }
                    Ok(())
                });

                // To record the client_name
                let client_name = Arc::new(RwLock::new(String::new()));
                // Spawn a receiver task.
                let receive_and_process =
                    {
                        let mut client_name = client_name.clone();
                        stream.for_each(move |(name, msg)| {
                            if client_name.read().unwrap().len() == 0 {
                                client_name.write().unwrap().push_str(&name);
                                let v: Vec<&str> = name.split(' ').collect();
                                if v[0] == "leader" {
                                    *user_tx.write().unwrap() = Some(tx.clone());
                                }
                                register_logic.clone()(v[0], v[1].parse::<u64>().unwrap());
                            } else {
                                if client_name.read().unwrap().contains("leader") {
                                    server_tx.send((name, msg)).unwrap();
                                }
                            }
                            Ok(())
                        })
                    };
                tokio::spawn(
                    send_to_client.select(receive_and_process)
                        .and_then(move |_| {
                            println!("{} disconnect", *client_name.read().unwrap());
                            Ok(())
                        }).map_err(|_| {})
                );
                Ok(())
            }).map_err(|e| { println!("{:?}", e); })
    )
}

//T is user message type
//U is to abstract tcp and uds
fn start_client<T, U>(connect: U,
                      client_name: String,
                      user_tx: Arc<RwLock<Option<mpsc::Sender<(String, T)>>>>,
                      client_tx: Box<std::sync::mpsc::Sender<(String, T)>>)
                      -> Box<Future<Item=(), Error=()> + Send + 'static>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone,
          U: Future + Send + Sync + 'static,
          <U as futures::Future>::Item: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync,
          <U as futures::Future>::Error: std::fmt::Debug + 'static + Send + Sync
{
    let (tx, rx): (mpsc::Sender<(String, T)>, mpsc::Receiver<(String, T)>) = mpsc::channel(0);
    let rx: Box<Stream<Item=(String, T), Error=io::Error> + Send> = Box::new(rx.map_err(|_| panic!()));
    *user_tx.write().unwrap() = Some(tx.clone());
    Box::new(
        connect.and_then(move |tcp_stream| {
            let message_codec: MessageCodec<T> = MessageCodec::new(client_name);
            let (sink, stream) = message_codec.framed(tcp_stream).split();

            // Spawn a sender task.
            let send_to_server = rx.forward(sink).then(|result| {
                if let Err(e) = result {
                    println!("failed to write to socket: {}", e)
                }
                Ok(())
            });

            // Spawn a receiver task.
            let receive_and_process = stream.for_each(move |(name, msg)| {
                client_tx.send((name, msg)).unwrap();
                Ok(())
            }).map_err(|e| { println!("faild to connect. err:{:?}", e); });

            tokio::spawn(send_to_server);
            tokio::spawn(receive_and_process);
            Ok(())
        }).map_err(|e| { println!("faild to connect. err:{:?}", e); })
    )
}

#[allow(dead_code)]
pub mod tcp;

#[allow(dead_code)]
pub mod uds;

pub fn create_tcp_fuzz_server<T>(addr: &str, server_name: &str) -> tcp::TcpFuzzServer<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    tcp::TcpFuzzServer::new(addr, server_name)
}

pub fn create_tcp_fuzz_client<T>(addr: &str, client_name: &str) -> tcp::TcpFuzzClient<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    tcp::TcpFuzzClient::new(addr, client_name)
}

pub fn create_uds_fuzz_server<T>(addr: &str, server_name: &str) -> uds::UdsFuzzServer<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    uds::UdsFuzzServer::new(addr, server_name)
}

pub fn create_uds_fuzz_client<T>(addr: &str, client_name: &str) -> uds::UdsFuzzClient<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    uds::UdsFuzzClient::new(addr, client_name)
}