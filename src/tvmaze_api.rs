use std::str::FromStr;

use hyper::{self, Client, StatusCode, Uri};
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;

use futures::{Future, Stream};
use tokio_core::reactor::Core;

use errors::*;

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
    pub fn new(verbose: bool) -> Result<Self> {
        let core = Core::new()?;
        let handle = core.handle();

        let connector = HttpsConnector::new(4, &handle)?;

        let client = Client::configure().connector(connector).build(&handle);

        Ok(Self {
            core: core,
            client: client,
            verbose: verbose,
        })
    }

    fn get_request(&self, uri: Uri) -> Box<Future<Item = hyper::Chunk, Error = ::errors::Error>> {
        let uri_str = uri.clone();
        let request = self.client.get(uri);
        let verbose = self.verbose;

        Box::new(
            request
                .map_err(|e| e.into())
                .and_then(move |res| {
                    if verbose {
                        println!("{} {}", res.status(), uri_str);
                    }

                    if res.status() != StatusCode::Ok {
                        return Err(ErrorKind::HttpError(res.status()).into());
                    }

                    Ok(res)
                })
                .and_then(|res| res.body().concat2().map_err(|e| e.into())),
        )
    }

    /// Searches TvMaze.com for shows with a given name.
    pub fn search_shows(&mut self, show: &str) -> Result<Vec<SearchResult>> {
        // Construct URI
        let uri = &format!("https://api.tvmaze.com/search/shows?q=\"{}\"", show);
        let uri = Uri::from_str(uri).chain_err(|| format!("Invalid URI [{}]", uri))?;

        // Send request and get response
        let response = self.get_request(uri);

        // Deserialize response into a Vec<SearchResult>
        let search_results = response.and_then(|body| {
            ::serde_json::from_slice(&body).chain_err(|| "Unable to deserialize HTTP response")
        });

        // Run future
        self.core
            .run(search_results)
            .chain_err(|| "HTTP request failed")
    }
}
