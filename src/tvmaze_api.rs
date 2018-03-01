use std::str::FromStr;
use std::fmt;
use std::cell::RefCell;
use std::cmp::Ordering;

use hyper::{self, Client, StatusCode, Uri};
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;

use futures::{Future, Stream};
use futures::stream::FuturesUnordered;
use tokio_core::reactor::Core;
use tokio_retry::RetryIf;
use tokio_retry::strategy::FibonacciBackoff;

use chrono::{DateTime, Utc};

use errors::*;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Network {
    pub id: usize,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Schedule {
    pub days: Vec<Day>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Status {
    #[serde(rename = "To Be Determined")]
    ToBeDetermined,
    #[serde(rename = "In Development")]
    InDevelopment,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Show {
    pub id: usize,
    pub name: String,
    pub language: Option<String>,
    pub network: Option<Network>,
    pub web_channel: Option<Network>,
    pub status: Status,
    pub runtime: Option<usize>,
    pub schedule: Schedule,
    #[serde(rename = "updated", default)]
    pub last_updated: u64,
    #[serde(default)]
    pub last_watched_episode: (usize, usize),
}

impl Ord for Show {
    fn cmp(&self, other: &Show) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for Show {
    fn partial_cmp(&self, other: &Show) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Show {
    fn eq(&self, other: &Show) -> bool {
        self.id == other.id
    }
}

impl Eq for Show {}

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
            self.network_name(),
            status,
            runtime
        )
    }
}

impl Show {
    pub fn network_name(&self) -> &str {
        match self.network {
            Some(ref network) => &network.name,
            None => match self.web_channel {
                Some(ref channel) => &channel.name,
                None => "Unknown",
            },
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SearchResult {
    pub score: f64,
    pub show: Show,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Episode {
    #[serde(rename = "id")]
    pub episode_id: usize,
    #[serde(default)]
    pub show_id: usize,
    pub name: String,
    pub season: usize,
    pub number: usize,
    pub airstamp: Option<DateTime<Utc>>,
    pub runtime: usize,
    #[serde(default)]
    pub watched: bool,
}

impl Ord for Episode {
    fn cmp(&self, other: &Episode) -> Ordering {
        if self.show_id == other.show_id {
            if self.season == other.season {
                self.number.cmp(&other.number)
            } else {
                self.season.cmp(&other.season)
            }
        } else {
            self.show_id.cmp(&other.show_id)
        }
    }
}

impl PartialOrd for Episode {
    fn partial_cmp(&self, other: &Episode) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Episode {
    fn eq(&self, other: &Episode) -> bool {
        self.episode_id == other.episode_id && self.show_id == other.show_id
    }
}

impl Eq for Episode {}

impl AsRef<Episode> for Episode {
    fn as_ref(&self) -> &Episode {
        self
    }
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

        if verbose {
            println!("GET {}", uri);
        }

        Box::new(request.map_err(|e| e.into()).and_then(move |res| {
            if verbose {
                println!("{} {}", res.status(), uri);
            }

            if res.status() != StatusCode::Ok {
                return Err(ErrorKind::HttpError(res.status(), uri).into());
            }

            Ok(res)
        }))
    }

    /// Make a GET request. Rate limiting of server is handled with retries.
    ///
    /// `&self` is moved into the returned future, therefore the future can't live longer
    /// than `&self`.
    fn make_get_request<'a>(
        &'a self,
        uri: Uri,
    ) -> Box<Future<Item = hyper::Chunk, Error = ::errors::Error> + 'a> {
        let retry_strategy = FibonacciBackoff::from_millis(1000).take(6);

        // TODO: use e.g. futures-poll-log crate to trace retry behaviour. I have the impression,
        //       something isn't behaving quite as it should..
        let retry_future = RetryIf::spawn(
            self.core.borrow().handle(),
            retry_strategy,
            move || self.create_get_request(uri.clone()),
            |e: &::errors::Error| match *e {
                Error(ErrorKind::HttpError(status, _), _) => status == StatusCode::TooManyRequests,
                _ => false,
            },
        );

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

    pub fn get_shows(&mut self, ids: &[usize]) -> Result<Vec<Show>> {
        let mut requests = FuturesUnordered::new();
        for id in ids {
            // Construct URI
            let uri = &format!("https://api.tvmaze.com/shows/{}", id);
            let uri = Uri::from_str(uri).chain_err(|| format!("Invalid URI [{}]", uri))?;

            // Send request and get response
            let response = self.make_get_request(uri);

            // Deserialize response into a Show
            let show = response.and_then(|body| {
                ::serde_json::from_slice::<Show>(&body)
                    .chain_err(|| "Unable to deserialize HTTP response")
            });

            // Queue request
            requests.push(show);
        }

        // Run future
        self.core
            .borrow_mut()
            .run(requests.collect())
            .chain_err(|| "HTTP request failed")
    }

    pub fn get_episodes(&mut self, ids: &[usize]) -> Result<Vec<Episode>> {
        let mut requests = FuturesUnordered::new();
        for id in ids {
            // Construct URI
            let uri = &format!("https://api.tvmaze.com/shows/{}/episodes", id);
            let uri = Uri::from_str(uri).chain_err(|| format!("Invalid URI [{}]", uri))?;

            // Send request and get response
            let response = self.make_get_request(uri);

            // Deserialize response into a Vec<Episode>
            let episodes = response.and_then(move |body| {
                let episodes: Result<Vec<Episode>> = ::serde_json::from_slice(&body)
                    .chain_err(|| "Unable to deserialize HTTP response");

                let mut episodes = match episodes {
                    Ok(episodes) => episodes,
                    Err(e) => return Err(e),
                };

                for episode in &mut episodes {
                    episode.show_id = *id;
                }

                Ok(episodes)
            });

            // Queue request
            requests.push(episodes);
        }

        // Run future
        self.core
            .borrow_mut()
            .run(requests.concat2())
            .chain_err(|| "HTTP request failed")
    }
}
