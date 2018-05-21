// extern crate crust;

#[macro_use]
extern crate clap;

use clap::{App, Arg, SubCommand};
// use crust::Config;

/**
 * TODO:
 * add
 * remove
 * list
 * find
 * vote
 * init
 */

macro_rules! subcommands {
  ($name:expr, $description:expr) => {
    SubCommand::with_name($name).about($description)
  };
}

fn main() {
  let matches = App::new(crate_name!())
    .author(crate_authors!("\n"))
    .about(crate_description!())
    .version(crate_version!())
    .subcommands(vec![
      subcommands!("list", "Show list of peers or data")
        .arg(Arg::with_name("PEER").short("p").long("peer"))
        .arg(Arg::with_name("DATA").short("d").long("data")),
      subcommands!("add", "Add peer or data")
        .arg(
          Arg::with_name("PEER")
            .short("p")
            .long("peer")
            .takes_value(true)
            .multiple(true),
        )
        .arg(
          Arg::with_name("DATA")
            .short("d")
            .long("data")
            .takes_value(true)
            .multiple(true),
        ),
    ])
    .get_matches();

  match matches.subcommand() {
    ("list", Some(m)) => {
      println!("{:?}", m);
    }
    _ => unreachable!(),
  };
}
