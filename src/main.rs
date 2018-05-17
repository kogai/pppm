#[macro_use]
extern crate serde_derive;

extern crate bigint;
extern crate bytes;
extern crate env_logger;
extern crate example;
extern crate futures;
extern crate libp2p_core as swarm;
extern crate libp2p_identify as identify;
extern crate libp2p_kad as kad;
extern crate libp2p_mplex as multiplex;
extern crate libp2p_peerstore as peerstore;
extern crate libp2p_secio as secio;
extern crate libp2p_tcp_transport as tcp;
extern crate serde;
extern crate serde_json;
extern crate tokio_core;
extern crate tokio_io;

mod kademlia;
mod store;

fn main() {
    env_logger::init();
    let store = store::Store::load("");
    println!("Hello, world! {:?}", store);
}
