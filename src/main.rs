extern crate hyper;
extern crate hyper_native_tls;
extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

use std::io::Read;
use std::error::Error;

use hyper::Client;
use hyper::status::StatusCode;
use hyper::net::HttpsConnector;

use hyper_native_tls::NativeTlsClient;

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
    let ssl = NativeTlsClient::new().unwrap();
    let connector = HttpsConnector::new(ssl);
    let client = Client::with_connector(connector);
    let url = "https://api.tvmaze.com/search/shows?q=\"the 100\"";
    let mut response = match client.get(url).send() {
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
            .enumerate() {
        let network_name = match result.show.network {
            Some(ref network) => network.name.clone(),
            None => {
                match result.show.web_channel {
                    Some(ref channel) => channel.name.clone(),
                    None => "Unknown".to_string(),
                }
            }
        };
        println!("[{}] {} ({})", i, result.show.name, network_name);
    }

    println!();
    println!("Credits: Data provided by TVmaze.com");
}
