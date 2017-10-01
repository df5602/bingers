use std::str::FromStr;
use std::error::Error;

use hyper::{Client, StatusCode, Uri};
use hyper_tls::HttpsConnector;

use futures::{future, Future, Stream};
use tokio_core::reactor::Core;

use serde_json;

#[derive(Debug, Deserialize)]
struct Network {
    id: usize,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Show {
    id: usize,
    name: String,
    language: String,
    network: Option<Network>,
    web_channel: Option<Network>,
    status: String,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    score: f64,
    show: Show,
}

pub struct App;

impl App {
    pub fn new() -> Self {
        Self {}
    }

    pub fn add_show(&self, show: &str) {
        let mut core = match Core::new() {
            Ok(core) => core,
            Err(e) => panic!("Unable to create core: {}", e.description()),
        };
        let handle = core.handle();

        let connector = match HttpsConnector::new(4, &handle) {
            Ok(connector) => connector,
            Err(e) => panic!("Unable to create HTTPS connector: {}", e.description()),
        };

        let client = Client::configure().connector(connector).build(&handle);

        let uri = match Uri::from_str(&format!(
            "https://api.tvmaze.com/search/shows?q=\"{}\"",
            show
        )) {
            Ok(uri) => uri,
            Err(e) => panic!("Invalid URI: {}", e.description()),
        };

        let request = client.get(uri).and_then(|res| {
            if res.status() != StatusCode::Ok {
                panic!("HTTPS Error: Received status {}", res.status());
            }

            res.body().concat2().and_then(|body| {
                let search_results: Vec<SearchResult> = match serde_json::from_slice(&body) {
                    Ok(search_results) => search_results,
                    Err(e) => panic!("Deserialization error: {}", e.description()),
                };

                future::ok::<_, _>(search_results)
            })
        });

        let search_results = match core.run(request) {
            Ok(response) => response,
            Err(e) => panic!("Unable to perform request: {}", e.description()),
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
}
