use std::str::FromStr;
use std::fmt;
use std::cell::RefCell;

use hyper::{self, Client, StatusCode, Uri};
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;

use futures::{Future, Stream};
use tokio_core::reactor::Core;
use tokio_retry::Retry;
use tokio_retry::strategy::FibonacciBackoff;

use errors::*;

#[derive(Debug, Deserialize)]
pub struct Network {
    id: usize,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub enum Day {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Deserialize)]
pub struct Schedule {
    pub days: Vec<Day>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub enum Status {
    #[serde(rename = "To Be Determined")] ToBeDetermined,
    #[serde(rename = "In Development")] InDevelopment,
    Running,
    Ended,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Status::ToBeDetermined => write!(f, "TBD"),
            Status::InDevelopment => write!(f, "In Development"),
            _ => write!(f, "{:?}", self),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Show {
    id: usize,
    pub name: String,
    pub language: String,
    pub network: Option<Network>,
    pub web_channel: Option<Network>,
    pub status: Status,
    pub runtime: Option<usize>,
    pub schedule: Schedule,
}

impl fmt::Display for Show {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let scheduled_days = if !self.schedule.days.is_empty() {
            if self.status == Status::Running {
                format!("{}s on ", self.schedule.days[0])
            } else {
                "".to_string()
            }
        } else {
            "".to_string()
        };

        let network_name = match self.network {
            Some(ref network) => network.name.clone(),
            None => match self.web_channel {
                Some(ref channel) => channel.name.clone(),
                None => "Unknown".to_string(),
            },
        };

        let status = match self.status {
            Status::Ended | Status::ToBeDetermined => format!(" ({})", self.status),
            _ => "".to_string(),
        };

        let runtime = match self.runtime {
            Some(runtime) => runtime,
            None => 0,
        };

        write!(
            f,
            "{} ({}{}{}, {}')",
            self.name,
            scheduled_days,
            network_name,
            status,
            runtime
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct SearchResult {
    pub score: f64,
    pub show: Show,
}

pub struct TvMazeApi {
    core: RefCell<Core>,
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
            core: RefCell::new(core),
            client: client,
            verbose: verbose,
        })
    }

    /// Create a new GET request.
    fn create_get_request(
        &self,
        uri: Uri,
    ) -> Box<Future<Item = hyper::Response, Error = ::errors::Error>> {
        let request = self.client.get(uri.clone());
        let verbose = self.verbose;

        Box::new(request.map_err(|e| e.into()).and_then(move |res| {
            if verbose {
                println!("{} {}", res.status(), uri);
            }

            if res.status() != StatusCode::Ok {
                return Err(ErrorKind::HttpError(res.status()).into());
            }

            Ok(res)
        }))
    }

    /// Make a GET request. Retry if an error occurs.
    ///
    /// `&self` is moved into the returned future, therefore the future can't live longer
    /// than `&self`.
    fn make_get_request<'a>(
        &'a self,
        uri: Uri,
    ) -> Box<Future<Item = hyper::Chunk, Error = ::errors::Error> + 'a> {
        let retry_strategy = FibonacciBackoff::from_millis(1000).take(6);

        let retry_future = Retry::spawn(self.core.borrow().handle(), retry_strategy, move || {
            self.create_get_request(uri.clone())
        });

        Box::new(
            retry_future
                .map_err(|e| e.into())
                .and_then(|res| res.body().concat2().map_err(|e| e.into())),
        )
    }

    /// Searches TvMaze.com for shows with a given name.
    pub fn search_shows(&mut self, show: &str) -> Result<Vec<SearchResult>> {
        // Construct URI
        let uri = &format!("https://api.tvmaze.com/search/shows?q=\"{}\"", show);
        let uri = Uri::from_str(uri).chain_err(|| format!("Invalid URI [{}]", uri))?;

        // Send request and get response
        let response = self.make_get_request(uri);

        // Deserialize response into a Vec<SearchResult>
        let search_results = response.and_then(|body| {
            ::serde_json::from_slice(&body).chain_err(|| "Unable to deserialize HTTP response")
        });

        // Run future
        // `self` is borrowed for the lifetime of the response future, which makes it
        // impossible to borrow `self` mutably here. The RefCell lets us get around this
        // restriction.
        self.core
            .borrow_mut()
            .run(search_results)
            .chain_err(|| "HTTP request failed")
    }
}
