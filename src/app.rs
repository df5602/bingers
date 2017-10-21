use errors::*;

use tvmaze_api::{Status, TvMazeApi};

pub struct App {
    api: TvMazeApi,
}

impl App {
    pub fn new() -> Result<Self> {
        Ok(Self {
            api: TvMazeApi::new(true)?,
        })
    }

    /// Add show to list of followed shows.
    ///
    /// Calls web API to search for shows with the given name.
    pub fn add_show(&mut self, show: &str) -> Result<()> {
        let search_results = self.api
            .search_shows(show)
            .chain_err(|| format!("Unable to search for show [\"{}\"]", show))?;

        println!();

        // TODO: make language user preference
        for (i, result) in search_results
            .iter()
            .filter(|result| {
                result.show.status == Status::Running || result.show.status == Status::Ended
                    || result.show.status == Status::ToBeDetermined
            })
            .filter(|result| result.show.language == "English")
            .enumerate()
        {
            println!("[{}] {}", i, result.show);
        }

        Ok(())
    }
}
