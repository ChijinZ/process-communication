extern crate tokio_uds;
extern crate tokio;
extern crate futures;
extern crate tokio_codec;
extern crate serde;
extern crate bincode;
extern crate bytes;

use super::*;
use std::path::Path;
use self::tokio_uds::{UnixStream, UnixListener};

#[derive(Debug)]
pub struct UdsFuzzServer<T> {
    // path is the Path bounded by UnixListener.
    // connetions is used to map client's name to sender of channel.
    pub path_name: String,
    pub name: String,
    phantom: PhantomData<T>,
}

impl<T> UdsFuzzServer<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    /// *addr* is socket address. like: 127.0.0.1:6666.
    ///
    /// *name* is the server's name, to identity which server it is.
    pub fn new(path_name: &str, server_name: &str) -> UdsFuzzServer<T> {
        UdsFuzzServer {
            path_name: String::from(path_name),
            name: String::from(server_name),
            phantom: PhantomData,
        }
    }

    pub fn start_server<F>(&self,
                           user_tx: Arc<RwLock<Option<mpsc::Sender<(String, T)>>>>,
                           server_tx: Box<std::sync::mpsc::Sender<(String, T)>>,
                           register_logic: F)
                           -> Box<Future<Item=(), Error=()> + Send + 'static>
        where F: FnMut(&str, u64) + Send + Sync + 'static + Clone
    {
        let path = Path::new(&self.path_name);
        let listener = UnixListener::bind(path)
            .expect("unable to bind Unix listener");
        start_server(listener.incoming(),
                     register_logic,
                     user_tx,
                     server_tx,
                     self.name.clone())
    }
}

#[derive(Debug)]
pub struct UdsFuzzClient<T> {
    // path_name is the Path connected by client.
    // phantom is just used to avoid compile error.
    path_name: String,
    name: String,
    phantom: PhantomData<T>,
}

impl<T> UdsFuzzClient<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    /// *addr* is socket address. like: 127.0.0.1:6666.
    /// *name* is the client's name, to identity which client it is.
    pub fn new(path_name: &str, client_name: &str) -> UdsFuzzClient<T> {
        UdsFuzzClient {
            path_name: String::from(path_name),
            name: String::from(client_name),
            phantom: PhantomData,
        }
    }

    pub fn start_client(&self,
                        user_tx: Arc<RwLock<Option<mpsc::Sender<(String, T)>>>>,
                        client_tx: Box<std::sync::mpsc::Sender<(String, T)>>)
                        -> Box<Future<Item=(), Error=()> + Send + 'static>
    {
        start_client(UnixStream::connect(Path::new(&self.path_name)),
                     self.name.clone(),
                     user_tx,
                     client_tx)
    }
}