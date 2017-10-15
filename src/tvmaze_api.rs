use std::str::FromStr;
use std::error::Error;

use hyper::{self, Client, StatusCode, Uri};
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;

use futures::{Future, Stream};
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
    verbose: bool,
}

impl TvMazeApi {
    pub fn new(verbose: bool) -> Self {
        let core = Core::new().unwrap();
        let handle = core.handle();

        let connector = HttpsConnector::new(4, &handle).unwrap();

        let client = Client::configure().connector(connector).build(&handle);

        Self {
            core: core,
            client: client,
            verbose: verbose,
        }
    }

    fn get_request(&self, uri: Uri) -> Box<Future<Item = hyper::Chunk, Error = hyper::Error>> {
        let uri_str = uri.clone();
        let request = self.client.get(uri);
        let verbose = self.verbose;

        Box::new(request.and_then(move |res| {
            if verbose {
                println!("{} {}", res.status(), uri_str);
            }

            if res.status() != StatusCode::Ok {
                panic!("HTTPS Error: Received status {}", res.status());
            }

            res.body().concat2()
        }))
    }

    /// Searches TvMaze.com for shows with a given name.
    pub fn search_shows(&mut self, show: &str) -> Vec<SearchResult> {
        // Construct URI
        let uri = match Uri::from_str(&format!(
            "https://api.tvmaze.com/search/shows?q=\"{}\"",
            show
        )) {
            Ok(uri) => uri,
            Err(e) => panic!("Invalid URI: {}", e.description()),
        };

        // Send request and get response
        let response = self.get_request(uri);

        // Deserialize response into a Vec<SearchResult>
        let search_results = response.and_then(|body| {
            let search_results: Vec<SearchResult> = match serde_json::from_slice(&body) {
                Ok(search_results) => search_results,
                Err(e) => panic!("Deserialization error: {}", e.description()),
            };
            Ok(search_results)
        });

        // Run future
        match self.core.run(search_results) {
            Ok(response) => response,
            Err(e) => panic!("Unable to perform request: {}", e.description()),
        }
    }
}
