extern crate clap;
extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate native_tls;
extern crate serde_json;
extern crate tokio_core;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate error_chain;

mod errors;
mod app;
mod tvmaze_api;

use clap::{Arg, SubCommand};

use errors::*;
use error_chain::ChainedError;

use app::App;

fn run(matches: &clap::ArgMatches) -> Result<()> {
    let mut app = App::new()?;

    // Dispatch to subcommands
    match matches.subcommand() {
        ("add", Some(m)) => {
            let show = m.value_of("tv_show").unwrap();
            app.add_show(show)?;
        }
        _ => {
            println!("{}", matches.usage());
            println!();
            println!("For more information try --help");
        }
    };

    Ok(())
}

fn main() {
    // Parse arguments
    let matches = clap::App::new("bingers")
        .version("0.1")
        .author("Dominik Fankhauser")
        .about("Manage your TV shows from the command line")
        .subcommand(
            SubCommand::with_name("add").about("Add TV show").arg(
                Arg::with_name("tv_show")
                    .required(true)
                    .index(1)
                    .value_name("SHOW"),
            ),
        )
        .after_help(
            "CREDITS:
    Data provided by TVmaze.com\n",
        )
        .get_matches();

    // Run app
    if let Err(ref e) = run(&matches) {
        println!("{}", e.display_chain().to_string());

        ::std::process::exit(1);
    }
}
