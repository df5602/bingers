use std::io::{self, Write};

use errors::*;
use tvmaze_api::{SearchResult, Show, Status, TvMazeApi};
use user_data::UserData;

pub struct App {
    api: TvMazeApi,
    user_data: UserData,
    verbose: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        Ok(Self {
            api: TvMazeApi::new(true)?,
            user_data: UserData::load()?,
            verbose: true,
        })
    }

    fn select_show(&self, search_results: &[SearchResult]) -> Result<Option<Show>> {
        // TODO: make language user preference
        for result in search_results
            .iter()
            .filter(|result| {
                result.show.status == Status::Running || result.show.status == Status::Ended
                    || result.show.status == Status::ToBeDetermined
            })
            .filter(|result| {
                if let Some(ref language) = result.show.language {
                    if language == "English" {
                        return true;
                    }
                }

                false
            }) {
            println!("Found:\n");
            println!("\t{}\n", result.show);
            print!("Add show? [y (yes); n (no); a (abort)] ");
            let _ = io::stdout().flush();

            let mut answer = String::new();
            io::stdin().read_line(&mut answer)?;

            match answer.as_str().trim() {
                "y" => {
                    return Ok(Some(result.show.clone()));
                }
                "n" => {}
                _ => {
                    println!("Aborted.");
                    return Ok(None);
                }
            }
        }

        if !search_results.is_empty() {
            println!("No more matching shows found.");
        } else {
            println!("No matching shows found.");
        }
        Ok(None)
    }

    /// Add show to list of followed shows.
    ///
    /// Calls web API to search for shows with the given name.
    pub fn add_show(&mut self, show: &str) -> Result<()> {
        let search_results = self.api
            .search_shows(show)
            .chain_err(|| format!("Unable to search for show [\"{}\"]", show))?;

        if self.verbose {
            println!();
        }

        let selected_show = self.select_show(&search_results)?;

        if let Some(show) = selected_show {
            println!("Added \"{}\"", show.name);
            self.user_data.add_show(show);
            self.user_data.store()?;
        }

        Ok(())
    }
}
