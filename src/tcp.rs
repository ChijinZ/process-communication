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

    pub fn start_server<F>(&self,
                           user_tx: Arc<RwLock<Option<mpsc::Sender<Option<T>>>>>,
                           server_tx: Box<std::sync::mpsc::Sender<Option<T>>>,
                           register_logic: F)
                           -> Box<Future<Item=(), Error=()> + Send + 'static>
        where F: FnMut(&str, u64) + Send + Sync + 'static + Clone
    {
        let listener = net::TcpListener::bind(&self.addr)
            .expect("unable to bind TCP listener");
        start_server(listener.incoming(),
                     register_logic,
                     user_tx,
                     server_tx,
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

    pub fn start_client(&self,
                        user_tx: Arc<RwLock<Option<mpsc::Sender<Option<T>>>>>,
                        client_tx: Box<std::sync::mpsc::Sender<Option<T>>>)
                        -> Box<Future<Item=(), Error=()> + Send + 'static>
    {
        start_client(net::TcpStream::connect(&self.connect_addr),
                     self.name.clone(),
                     user_tx,
                     client_tx)

        //tokio::run(done);
    }
}