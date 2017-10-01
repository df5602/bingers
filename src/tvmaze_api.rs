use std::str::FromStr;
use std::error::Error;

use hyper::{Client, StatusCode, Uri};
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;

use futures::{future, Future, Stream};
use tokio_core::reactor::Core;

use serde_json;

#[derive(Debug, Deserialize)]
pub struct Network {
    id: usize,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Show {
    id: usize,
    pub name: String,
    pub language: String,
    pub network: Option<Network>,
    pub web_channel: Option<Network>,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchResult {
    pub score: f64,
    pub show: Show,
}

pub struct TvMazeApi {
    core: Core,
    client: Client<HttpsConnector<HttpConnector>>,
}

impl TvMazeApi {
    pub fn new() -> Self {
        let core = Core::new().unwrap();
        let handle = core.handle();

        let connector = HttpsConnector::new(4, &handle).unwrap();

        let client = Client::configure().connector(connector).build(&handle);

        Self {
            core: core,
            client: client,
        }
    }

    pub fn search_shows(&mut self, show: &str) -> Vec<SearchResult> {
        let uri = match Uri::from_str(&format!(
            "https://api.tvmaze.com/search/shows?q=\"{}\"",
            show
        )) {
            Ok(uri) => uri,
            Err(e) => panic!("Invalid URI: {}", e.description()),
        };

        let request = self.client.get(uri).and_then(|res| {
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

        match self.core.run(request) {
            Ok(response) => response,
            Err(e) => panic!("Unable to perform request: {}", e.description()),
        }
    }
}
