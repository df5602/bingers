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
        ("watched", Some(m)) => {
            let show = m.value_of("tv_show").unwrap();

            let season = match m.value_of("season") {
                Some(season) => Some(season.parse::<usize>()?),
                None => None,
            };

            let episode = match m.value_of("episode") {
                Some(episode) => Some(episode.parse::<usize>()?),
                None => None,
            };

            app.mark_as_watched(show, season, episode)?;
        }
        ("update", Some(m)) => {
            let force = m.is_present("force");
            app.update(force)?;
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
        .subcommand(
            SubCommand::with_name("watched")
                .about(
                    "Mark episode as watched\n
If not specified otherwise, will mark the next unwatched episode as watched.
Use the --season and --episode arguments to override.",
                )
                .arg(
                    Arg::with_name("tv_show")
                        .required(true)
                        .index(1)
                        .value_name("SHOW"),
                )
                .arg(
                    Arg::with_name("season")
                        .short("s")
                        .long("season")
                        .takes_value(true)
                        .help(
                            "Specify season. \
                             If used without --episode, will mark whole season as watched.",
                        ),
                )
                .arg(
                    Arg::with_name("episode")
                        .short("e")
                        .long("episode")
                        .takes_value(true)
                        .requires("season")
                        .help("Specify episode"),
                ),
        )
        .subcommand(
            SubCommand::with_name("update")
                .about("Update TV shows and episodes")
                .arg(
                    Arg::with_name("force")
                        .short("f")
                        .long("force")
                        .help("Force update of all shows and episodes"),
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
