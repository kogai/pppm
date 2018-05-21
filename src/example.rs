extern crate clap;
extern crate future_utils;
extern crate futures;
extern crate rand;
#[macro_use]
extern crate rand_derive;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tokio_core;
#[macro_use]
extern crate unwrap;
extern crate void;

extern crate crust;

mod utils;

use clap::App;
use crust::{ConfigFile, Service};
use future_utils::bi_channel;
use futures::Stream;
use futures::future::{empty, Future};
use rand::Rng;
use tokio_core::reactor::Core;
use utils::PeerId;

fn main() {
  let _ = App::new("Crust basic connection example")
    .about(
      "Attempts to connect to remote peer given its connection information. \
       Start two instances of this example. Each instance generates and prints its \
       connection information to stdout in JSON format. You have to manually copy/paste \
       this info from one instance to the other and hit ENTER to start connection.",
    )
    .get_matches();

  let mut event_loop = unwrap!(Core::new());
  let handle = event_loop.handle();
  let service_id = rand::thread_rng().gen::<PeerId>();
  println!("Service id: {}", service_id);

  let config = unwrap!(ConfigFile::new_temporary());
  unwrap!(config.write()).listen_addresses = vec![
    unwrap!("tcp://0.0.0.0:0".parse()),
    unwrap!("utp://0.0.0.0:0".parse()),
  ];
  let make_service = Service::with_config(&event_loop.handle(), config, service_id);
  let service = unwrap!(
    event_loop.run(make_service),
    "Failed to create Service object",
  );

  let listeners = unwrap!(
    event_loop.run(service.start_listening().collect()),
    "Failed to start listening to peers",
  );
  for listener in &listeners {
    println!("Listening on {}", listener.addr());
  }

  let (ci_channel1, ci_channel2) = bi_channel::unbounded();
  utils::exchange_conn_info(&handle, ci_channel2);

  let connect = service
    .connect(ci_channel1)
    .map(move |peer| (peer, service));
  let (peer, _service) = unwrap!(event_loop.run(connect));
  println!(
    "Connected to peer: {} - {}",
    peer.uid(),
    unwrap!(peer.addr())
  );

  // Run event loop forever.
  let res = event_loop.run(empty::<(), ()>());
  unwrap!(res);
}
