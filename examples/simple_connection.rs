extern crate tokio;
extern crate futures;
extern crate process_communication;

use process_communication::*;
use tokio::prelude::*;
use futures::sync::mpsc as future_mpsc;
use std::sync::mpsc as std_mpsc;
use std::env;
use std::sync::{Arc, RwLock};

fn main() {
    let a = env::args().skip(1).collect::<Vec<_>>();
    match a.first().unwrap().as_str() {
        "client" => client(),
        "server" => server(),
        _ => panic!("failed"),
    };
}

fn server() {
    let mut user_tx:Arc<RwLock<Option<future_mpsc::Sender<Option<u32>>>>> = Arc::new(RwLock::new(None));
    let (server_tx, user_rx): (std_mpsc::Sender<Option<u32>>, std_mpsc::Receiver<Option<u32>>) = std_mpsc::channel();
    let server: tcp::TcpFuzzServer<u32> = create_tcp_fuzz_server("127.0.0.1:6666", "server");
    let server_task =
        server.start_server(user_tx.clone(),
                            Box::new(server_tx),
                            |name, pid| {
                                println!("{},{}", name, pid);
                            });
    std::thread::spawn(|| {
        tokio::run(server_task);
    });
    std::thread::sleep_ms(10000);
    let x = user_tx.read().unwrap().clone().unwrap().try_send(Some(666)).unwrap();
    std::thread::sleep_ms(10000);
}

fn client() {
    let user_tx = Arc::new(RwLock::new(None));
    let (client_tx, user_rx): (std_mpsc::Sender<Option<u32>>, std_mpsc::Receiver<Option<u32>>) = std_mpsc::channel();
    let client: tcp::TcpFuzzClient<u32> = create_tcp_fuzz_client("127.0.0.1:6666", format!("leader {}", std::process::id()).as_str());
    let client_task =
        client.start_client(user_tx,
                            Box::new(client_tx));
    std::thread::spawn(move || {
    tokio::run(client_task);
    });
    println!("{}", user_rx.recv().unwrap().unwrap());
}