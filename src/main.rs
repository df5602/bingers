extern crate clap;
extern crate hyper;
extern crate hyper_native_tls;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

use std::io::Read;
use std::error::Error;

use hyper::Client;
use hyper::status::StatusCode;
use hyper::net::HttpsConnector;

use hyper_native_tls::NativeTlsClient;

use clap::{App, Arg, SubCommand};

#[derive(Debug, Serialize, Deserialize)]
struct Network {
    id: usize,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Show {
    id: usize,
    name: String,
    language: String,
    network: Option<Network>,
    web_channel: Option<Network>,
    status: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SearchResult {
    score: f64,
    show: Show,
}

fn main() {
    let matches = App::new("bingers")
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
        .get_matches();

    let show = match matches.subcommand() {
        ("add", Some(m)) => m.value_of("tv_show").unwrap(),
        _ => unimplemented!(),
    };

    let ssl = NativeTlsClient::new().unwrap();
    let connector = HttpsConnector::new(ssl);
    let client = Client::with_connector(connector);
    let url = format!("https://api.tvmaze.com/search/shows?q=\"{}\"", show);
    let mut response = match client.get(&url).send() {
        Ok(response) => {
            if response.status != StatusCode::Ok {
                panic!("HTTP Error: Received status {}", response.status);
            }
            response
        }
        Err(e) => panic!("Error: {}", e.description()),
    };

    let mut buf = String::new();
    match response.read_to_string(&mut buf) {
        Ok(_) => (),
        Err(e) => panic!("Error: {}", e.description()),
    };

    //println!("{:#}", buf);

    let search_results: Vec<SearchResult> = match serde_json::from_str(&buf) {
        Ok(search_results) => search_results,
        Err(e) => panic!("Deserialization Error: {}", e.description()),
    };

    //TODO: provide unfiltered view as fallback
    for (i, result) in search_results
        .iter()
        .filter(|result| result.show.status == "Running")
        .filter(|result| result.show.language == "English")
        .enumerate()
    {
        let network_name = match result.show.network {
            Some(ref network) => network.name.clone(),
            None => match result.show.web_channel {
                Some(ref channel) => channel.name.clone(),
                None => "Unknown".to_string(),
            },
        };
        println!("[{}] {} ({})", i, result.show.name, network_name);
    }

    println!();
    println!("Credits: Data provided by TVmaze.com");
}
