extern crate app_dirs;
extern crate chrono;
extern crate clap;
extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate native_tls;
extern crate serde_json;
extern crate tokio_core;
extern crate tokio_retry;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate error_chain;

mod errors;
mod tvmaze_api;
mod user_data;
mod app;

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
        ("list", Some(m)) => if m.is_present("shows") {
            app.list_shows()?;
        } else {
            app.list_episodes()?;
        },
        ("remove", Some(m)) => {
            let show = m.value_of("tv_show").unwrap();
            app.remove_show(show)?;
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
        .subcommand(
            SubCommand::with_name("list")
                .about(
                    "List TV shows or episodes\n
When no flag is given, episodes will be listed.",
                )
                .arg(
                    Arg::with_name("shows")
                        .short("s")
                        .long("shows")
                        .help("List shows"),
                )
                .arg(
                    Arg::with_name("episodes")
                        .short("e")
                        .long("episodes")
                        .conflicts_with("shows")
                        .help("List episodes (default)"),
                ),
        )
        .subcommand(
            SubCommand::with_name("remove").about("Remove TV show").arg(
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
