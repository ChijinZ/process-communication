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
    let mut user_tx = Arc::new(RwLock::new(None));
    let (server_tx, user_rx) = std_mpsc::channel();
    let server: uds::UdsFuzzServer<u32> = create_uds_fuzz_server("./a", "server");
    let server_task =
        server.start_server(user_tx.clone(),
                            Box::new(server_tx),
                            |name, pid| {
                                println!("{},{}", name, pid);
                            });
    std::thread::spawn(|| {
        tokio::run(server_task);
    });
    std::thread::sleep_ms(5000);
    while true {
        let (name, msg) = user_rx.recv().unwrap();
        println!("{}", msg);
        user_tx.read().unwrap().clone().unwrap().try_send((name.clone(), msg + 1)).unwrap();
    }
}

fn client() {
    let user_tx = Arc::new(RwLock::new(None));
    let (client_tx, user_rx) = std_mpsc::channel();
    let name = format!("leader {}", std::process::id());
    let client: uds::UdsFuzzClient<u32> = create_uds_fuzz_client("./a", name.as_str());
    let client_task =
        client.start_client(user_tx.clone(),
                            Box::new(client_tx));
    std::thread::spawn(move || {
        tokio::run(client_task);
    });
    user_tx.read().unwrap().clone().unwrap().try_send((name.clone(), 0)).unwrap();
    std::thread::sleep_ms(10);
    user_tx.read().unwrap().clone().unwrap().try_send((name.clone(), 0)).unwrap();
    use std::time::{Duration, Instant};
    let instant = Instant::now();
    let how_much_time = Duration::from_secs(10);
    while instant.elapsed() < how_much_time {
        match user_rx.recv() {
            Ok((name, msg)) => {
                println!("{}", msg);
                user_tx.read().unwrap().clone().unwrap().try_send((name.clone(), msg + 1)).unwrap();
            }
            Err(_) =>{
                println!("fuck");
            }
        }
    }
}