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

    fn select_show_to_add(&self, search_results: &[SearchResult]) -> Result<Option<Show>> {
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

    fn select_show_to_remove<'a>(&self, candidates: &'a [&Show]) -> Result<Option<&'a Show>> {
        for candidate in candidates {
            println!("Found:\n");
            println!("\t{}\n", candidate);
            print!("Remove show? [y (yes); n (no); a (abort)] ");
            let _ = io::stdout().flush();

            let mut answer = String::new();
            io::stdin().read_line(&mut answer)?;

            match answer.as_str().trim() {
                "y" => {
                    return Ok(Some(candidate));
                }
                "n" => {}
                _ => {
                    println!("Aborted.");
                    return Ok(None);
                }
            }
        }

        println!("No more matching shows found.");
        Ok(None)
    }

    fn match_with_subscribed_shows<'a>(&self, search_results: &'a [SearchResult]) -> Vec<&'a Show> {
        let mut matched_shows = Vec::new();
        let subscribed_shows = self.user_data.subscribed_shows();

        for matched_show in search_results
            .iter()
            .filter(|matched_show| subscribed_shows.contains(&matched_show.show))
        {
            matched_shows.push(&matched_show.show);
        }

        matched_shows
    }

    /// Add show to list of subscribed shows.
    ///
    /// Calls web API to search for shows with the given name.
    pub fn add_show(&mut self, show: &str) -> Result<()> {
        let search_results = self.api
            .search_shows(show)
            .chain_err(|| format!("Unable to search for show [\"{}\"]", show))?;

        if self.verbose {
            println!();
        }

        let selected_show = self.select_show_to_add(&search_results)?;

        if let Some(show) = selected_show {
            println!("Added \"{}\"", show.name);
            self.user_data.add_show(show);
            self.user_data.store()?;
        }

        Ok(())
    }

    /// Remove show from list of subscribed shows.
    ///
    /// Calls web API to search for shows with the given name, then matches the results against
    /// the list of subscribed shows.
    pub fn remove_show(&mut self, show: &str) -> Result<()> {
        let search_results = self.api
            .search_shows(show)
            .chain_err(|| format!("Unable to search for show [\"{}\"]", show))?;

        let matched_shows = self.match_with_subscribed_shows(&search_results);

        if matched_shows.is_empty() {
            return Ok(());
        }

        let show_to_remove = if matched_shows.len() > 1 {
            match self.select_show_to_remove(&matched_shows)? {
                Some(show) => show,
                None => return Ok(()),
            }
        } else {
            matched_shows[0]
        };

        if self.verbose {
            println!();
        }

        // TODO: maybe don't delete episode information in user data, but move to
        //       archived section.. In case show is removed in error, one could simply
        //       re-add it.

        println!("Removed \"{}\"", show_to_remove);
        self.user_data.remove_show(show_to_remove);
        self.user_data.store()?;

        Ok(())
    }

    /// List all followed shows
    pub fn list_shows(&self) -> Result<()> {
        let subscribed_shows = self.user_data.subscribed_shows();

        if subscribed_shows.is_empty() {
            println!("You have not subscribed to any shows.");
            return Ok(());
        }

        // TODO: sorting order? (sort by date of most recent episode?)
        // TODO: different formatting?
        println!("Subscribed shows:");
        println!();

        for show in subscribed_shows
            .iter()
            .filter(|show| show.status == Status::Running)
        {
            println!("\t{}", show);
        }

        for show in subscribed_shows
            .iter()
            .filter(|show| show.status != Status::Running)
        {
            println!("\t{}", show);
        }

        Ok(())
    }
}
