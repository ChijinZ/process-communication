extern crate tokio;
extern crate futures;
extern crate tokio_codec;
extern crate serde;
extern crate bincode;
extern crate bytes;

use std::net::SocketAddr;

use super::*;

#[derive(Debug)]
pub struct TcpFuzzServer<T> {
    // addr is the socket address which server will bind and listen to.
    // connetions is used to map client's name to sender of channel.
    pub addr: SocketAddr,
    pub name: String,
    phantom: PhantomData<T>,
}

impl<T> TcpFuzzServer<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    /// *addr* is socket address. like: 127.0.0.1:6666.
    ///
    /// *name* is the server's name, to identity which server it is.
    pub fn new(addr: &str, server_name: &str) -> TcpFuzzServer<T> {
        let socket_addr = addr.parse::<SocketAddr>().unwrap();
        TcpFuzzServer {
            addr: socket_addr,
            name: String::from(server_name),
            phantom: PhantomData,
        }
    }

    /// *first_msg* is the first message that server send to the client which just
    /// connect to server.
    ///
    /// *process_fuction* receive a tuple of <client_name, message>, and return
    /// a series of tuple of (client_name,message) indicating which message will be
    /// sent to which client. Note that if you want to send a message to current
    /// client, you should set client_name as a string with 0 length, i.e. "" .
    pub fn start_server<F>(&self, first_msg: T,
                           process_function: F)
                           -> Box<Future<Item=(), Error=()> + Send + 'static>
        where F: FnMut(String, T) -> Vec<T> + Send + Sync + 'static + Clone
    {
        let listener = net::TcpListener::bind(&self.addr)
            .expect("unable to bind TCP listener");
        start_server(listener.incoming(),
                     first_msg,
                     process_function,
                     self.name.clone())
        //tokio::run(done);
    }
}

#[derive(Debug)]
pub struct TcpFuzzClient<T> {
    // addr is the socket address which client will connect to.
    // phantom is just used to avoid compile error.
    connect_addr: SocketAddr,
    name: String,
    phantom: PhantomData<T>,
}

impl<T> TcpFuzzClient<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    /// *addr* is socket address. like: 127.0.0.1:6666.
    ///
    /// *name* is the client's name, to identity which client it is.
    pub fn new(addr: &str, client_name: &str) -> TcpFuzzClient<T> {
        let socket_addr = addr.parse::<SocketAddr>().unwrap();
        TcpFuzzClient {
            connect_addr: socket_addr,
            name: String::from(client_name),
            phantom: PhantomData,
        }
    }

    /// process_function receive a message from server and send a series
    /// of messages to server
    pub fn start_client<F>(&self, process_function: F) -> Box<Future<Item=(), Error=()> + Send + 'static>
        where F: FnMut(T) -> Vec<T> + Send + Sync + 'static
    {
        start_client(net::TcpStream::connect(&self.connect_addr),
                     self.name.clone(),
                     process_function)

        //tokio::run(done);
    }
}