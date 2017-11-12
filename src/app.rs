use std::io::{self, Write};
use std::collections::HashMap;
use std::cmp::max;

use chrono::Utc;

use errors::*;
use tvmaze_api::{Episode, SearchResult, Show, Status, TvMazeApi};
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

    fn print_episode_list_as_table(episodes: &[Episode]) {
        // Calculate maximum length of episode name
        let max_length = episodes
            .iter()
            .map(|episode| episode.name.len())
            .fold(0, |max, length| if length > max { length } else { max });

        println!(
            "Season | Episode | {: <width$} | Air Date",
            "Name",
            width = max_length
        );

        let hline = format!(
            "-------|---------|-{:-<width$}-|-------------------",
            "-",
            width = max_length
        );
        println!("{}", hline);

        let mut current_season = 1;
        for episode in episodes {
            if episode.season > current_season {
                current_season = episode.season;

                println!("{}", hline);
            }

            let air_date = match episode.airstamp {
                Some(airstamp) => format!("{}", airstamp.format("%a, %b %d, %Y")),
                None => "TBD".to_string(),
            };

            println!(
                "{: >6} | {: >7} | {: <width$} | {}",
                episode.season,
                episode.number,
                episode.name,
                air_date,
                width = max_length
            );
        }
    }

    fn print_show_list_as_table(shows: &[&Show], unwatched_episode_count: &HashMap<usize, usize>) {
        // Calculate maximum length of show and network name
        let (max_length_name, max_length_network) = shows
            .iter()
            .map(|show| (show.name.len(), show.network_name().len()))
            .fold(
                (0, 0),
                |(max_length_name, max_length_network), (length_name, length_network)| {
                    (
                        max(max_length_name, length_name),
                        max(max_length_network, length_network),
                    )
                },
            );

        // Print header
        println!(
            "{: <width_show_name$} | {: <width_network_name$} | {: <14} |",
            "Name",
            "Network",
            "Status",
            width_show_name = max_length_name,
            width_network_name = max_length_network,
        );
        println!(
            "{:-<width_show_name$}-|-{:-<width_network_name$}-|-{:-<14}-|------------------------",
            "-",
            "-",
            "-",
            width_show_name = max_length_name,
            width_network_name = max_length_network,
        );

        // Print shows
        for show in shows {
            let status = &format!("{}", show.status);

            let unwatched = match unwatched_episode_count.get(&show.id) {
                Some(count) => if *count > 1 {
                    format!("{} unwatched episodes", count)
                } else {
                    assert_eq!(1, *count);
                    "1 unwatched episode".to_string()
                },
                None => "".to_string(),
            };

            println!(
                "{: <width_show_name$} | {: <width_network_name$} | {: <14} | {}",
                show.name,
                show.network_name(),
                status,
                unwatched,
                width_show_name = max_length_name,
                width_network_name = max_length_network,
            );
        }
    }

    fn get_episodes(&mut self, show: &Show) -> Result<(Vec<Episode>, (usize, usize))> {
        print!(
            "Have you already watched some episodes of {}? [y (yes); n (no)] ",
            show.name
        );
        let _ = io::stdout().flush();

        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;

        let mut episodes = self.api.get_episodes(show.id)?;

        // Remove episodes that haven't aired yet
        episodes.retain(|episode| match episode.airstamp {
            Some(airstamp) => Utc::now() >= airstamp,
            None => false,
        });

        if self.verbose {
            println!();
        }

        let (season, number) = match answer.as_str().trim() {
            "y" | "yes" => {
                App::print_episode_list_as_table(&episodes);
                println!();
                println!("Specify the last episode you have watched:");

                print!("Season: ");
                let _ = io::stdout().flush();
                answer.clear();
                io::stdin().read_line(&mut answer)?;
                let season: usize = answer.trim().parse()?;

                print!("Episode: ");
                let _ = io::stdout().flush();
                answer.clear();
                io::stdin().read_line(&mut answer)?;
                let episode: usize = answer.trim().parse()?;

                (season, episode)
            }
            _ => (0, 0),
        };

        // Only keep episodes that haven't been watched yet
        episodes.retain(|episode| {
            episode.season >= season && episode.number > number
        });

        Ok((episodes, (season, number)))
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

        if let Some(mut show) = selected_show {
            println!("Added \"{}\"", show.name);
            println!();
            let (episodes, last_watched) = self.get_episodes(&show)?;

            // BUG: When selecting last episode of previous season as last watched, no shows
            //      get added?

            // Fill in information about last watched episode
            show.last_watched_episode = last_watched;

            // Add to user data
            self.user_data.add_show(show);
            self.user_data.add_episodes(episodes);
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

        println!("Removed \"{}\"", show_to_remove);
        self.user_data.remove_episodes(show_to_remove);
        self.user_data.remove_show(show_to_remove);
        self.user_data.store()?;

        Ok(())
    }

    /// List all followed shows
    pub fn list_shows(&mut self) -> Result<()> {
        let subscribed_shows = self.user_data.subscribed_shows_by_most_recent();

        if subscribed_shows.is_empty() {
            println!("You have not subscribed to any shows.");
            return Ok(());
        }

        let mut unwatched_episode_count: HashMap<usize, usize> = HashMap::new();

        let unwatched_episodes = self.user_data.unwatched_episodes();
        for episode in unwatched_episodes {
            *unwatched_episode_count.entry(episode.show_id).or_insert(0) += 1;
        }

        println!("Subscribed shows:");
        println!();

        App::print_show_list_as_table(&subscribed_shows, &unwatched_episode_count);
        println!();

        Ok(())
    }
}
