use std::cmp::{max, Ordering};
use std::collections::HashMap;
use std::io::{self, Write};

use chrono::{Datelike, Utc};

use errors::*;
use tvmaze_api::{Episode, SearchResult, Show, Status, TvMazeApi};
use user_data::UserData;

#[derive(PartialEq)]
enum HorizontalSeparator {
    Season,
    Week,
}

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

    fn select_show<'a>(&self, candidates: &'a [&Show]) -> Result<Option<&'a Show>> {
        for candidate in candidates {
            println!("Found:\n");
            println!("\t{}\n", candidate);
            print!("Did you mean this show? [y (yes); n (no); a (abort)] ");
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

    // TODO: refactor such that this actually works with the borrow checker
    //
    // fn resolve_show(&mut self, show: &str) -> Result<Option<Show>> {
    //     let search_results = self.api
    //         .search_shows(show)
    //         .chain_err(|| format!("Unable to search for show [\"{}\"]", show))?;

    //     let selected_show = {
    //         let matched_shows = self.match_with_subscribed_shows(&search_results);

    //         if matched_shows.is_empty() {
    //             println!("No matching show found.");
    //             return Ok(None);
    //         }

    //         if matched_shows.len() > 1 {
    //             match self.select_show(&matched_shows)? {
    //                 Some(show) => show,
    //                 None => {
    //                     println!("No matching show found.");
    //                     return Ok(None);
    //                 }
    //             }
    //         } else {
    //             matched_shows[0]
    //         }
    //     };

    //     Ok(Some(*selected_show))
    // }

    #[allow(unknown_lints)]
    #[allow(print_literal)] // see clippy issue #2634
    fn print_episode_list_as_table<T: AsRef<Episode>>(
        episodes: &[T],
        separator: &HorizontalSeparator,
        show_names: Option<&HashMap<usize, &str>>,
    ) {
        // Calculate maximum length of episode name
        let max_ep_length = episodes
            .iter()
            .filter(|episode| !episode.as_ref().watched)
            .map(|episode| episode.as_ref().name.len())
            .fold(0, max);

        // If applicable, calculate maximum length of show name
        let max_show_length = if let Some(show_names) = show_names {
            episodes
                .iter()
                .filter(|episode| !episode.as_ref().watched)
                .map(|episode| {
                    if let Some(name) = show_names.get(&episode.as_ref().show_id) {
                        name.len()
                    } else {
                        0
                    }
                })
                .fold(0, max)
        } else {
            0
        };

        if max_show_length > 0 {
            print!("{: <width$} | ", "Show", width = max_show_length);
        }
        println!(
            "Season | Episode | {: <width$} | Air Date",
            "Name",
            width = max_ep_length
        );

        let mut hline = if max_show_length > 0 {
            format!("{:-<width$}-|-", "-", width = max_show_length)
        } else {
            "".to_string()
        };

        hline.push_str(&format!(
            "-------|---------|-{:-<width$}-|-------------------",
            "-",
            width = max_ep_length
        ));

        println!("{}", hline);

        let mut current_season = 1;
        let mut current_week: u32 = 0;
        for (i, episode) in episodes
            .iter()
            .filter(|episode| !episode.as_ref().watched)
            .enumerate()
        {
            let episode = episode.as_ref();

            let this_week = if let Some(airdate) = episode.airstamp {
                airdate.iso_week().week()
            } else {
                0
            };

            if i > 0
                && ((separator == &HorizontalSeparator::Season && episode.season != current_season)
                    || (separator == &HorizontalSeparator::Week && this_week != current_week))
            {
                println!("{}", hline);
            }
            current_season = episode.season;
            current_week = this_week;

            let air_date = match episode.airstamp {
                Some(airstamp) => format!("{}", airstamp.format("%a, %b %d, %Y")),
                None => "TBD".to_string(),
            };

            if let Some(show_names) = show_names {
                let name = if let Some(name) = show_names.get(&episode.show_id) {
                    name
                } else {
                    "???"
                };

                print!("{: <width$} | ", name, width = max_show_length);
            };

            println!(
                "{: >6} | {: >7} | {: <width$} | {}",
                episode.season,
                episode.number,
                episode.name,
                air_date,
                width = max_ep_length
            );
        }
    }

    #[allow(unknown_lints)]
    #[allow(print_literal)] // see clippy issue #2634
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

        let show_ids = [show.id];
        let mut episodes = self.api.get_episodes(&show_ids)?;

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
                App::print_episode_list_as_table(&episodes, &HorizontalSeparator::Season, None);
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
            if episode.season == season {
                episode.number > number
            } else {
                episode.season > season
            }
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
            println!("Added \"{}\".", show.name);
            println!();
            let (episodes, last_watched) = self.get_episodes(&show)?;

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
            println!("No matching show found.");
            return Ok(());
        }

        let show_to_remove = if matched_shows.len() > 1 {
            match self.select_show(&matched_shows)? {
                Some(show) => show,
                None => {
                    println!("No matching show found.");
                    return Ok(());
                }
            }
        } else {
            matched_shows[0]
        };

        if self.verbose {
            println!();
        }

        println!("Removed \"{}\".", show_to_remove);
        self.user_data.remove_episodes(show_to_remove);
        self.user_data.remove_show(show_to_remove);
        self.user_data.store()?;

        Ok(())
    }

    /// List all followed shows
    pub fn list_shows(&self) -> Result<()> {
        let subscribed_shows = self.user_data.subscribed_shows_by_most_recent();

        if subscribed_shows.is_empty() {
            println!("You have not subscribed to any shows.");
            return Ok(());
        }

        let mut unwatched_episode_count: HashMap<usize, usize> = HashMap::new();

        let unwatched_episodes = self.user_data.unwatched_episodes();
        for episode in unwatched_episodes.iter().filter(|episode| !episode.watched) {
            *unwatched_episode_count.entry(episode.show_id).or_insert(0) += 1;
        }

        println!("Subscribed shows:");
        println!();

        App::print_show_list_as_table(&subscribed_shows, &unwatched_episode_count);
        println!();

        Ok(())
    }

    /// List all unwatched episodes
    pub fn list_episodes(&self) -> Result<()> {
        let episodes = self.user_data.unwatched_episodes_oldest_first();

        if episodes.is_empty() {
            println!("You have no unwatched episodes!");
            return Ok(());
        }

        let mut show_names: HashMap<usize, &str> = HashMap::new();

        let shows = self.user_data.subscribed_shows();
        for show in shows {
            show_names.insert(show.id, &show.name);
        }

        println!("Unwatched episodes:");
        println!();

        App::print_episode_list_as_table(&episodes, &HorizontalSeparator::Week, Some(&show_names));
        println!();

        Ok(())
    }

    /// Mark episode(s) as watched
    pub fn mark_as_watched(
        &mut self,
        show: &str,
        season: Option<usize>,
        episode: Option<usize>,
    ) -> Result<()> {
        let search_results = self.api
            .search_shows(show)
            .chain_err(|| format!("Unable to search for show [\"{}\"]", show))?;

        let matched_shows = self.match_with_subscribed_shows(&search_results);

        if matched_shows.is_empty() {
            println!("No matching show found.");
            return Ok(());
        }

        let show_to_update = if matched_shows.len() > 1 {
            match self.select_show(&matched_shows)? {
                Some(show) => show,
                None => {
                    println!("No matching show found.");
                    return Ok(());
                }
            }
        } else {
            matched_shows[0]
        };

        if self.verbose {
            println!();
        }

        let last_marked = self.user_data
            .mark_as_watched(show_to_update.id, season, episode);

        if let Some(last_marked) = last_marked {
            if let Some(season) = season {
                if let Some(episode) = episode {
                    println!(
                        "Marked season {} episode {} of {} as watched.",
                        season, episode, show_to_update.name
                    );
                } else {
                    println!(
                        "Marked season {} of {} as watched.",
                        season, show_to_update.name
                    );
                }
            } else {
                println!(
                    "Marked season {} episode {} of {} as watched.",
                    last_marked.0, last_marked.1, show_to_update.name
                );
            }

            self.user_data.store()?;
        }

        Ok(())
    }

    /// Update TV shows and episodes
    pub fn update(&mut self, force: bool) -> Result<()> {
        // Get TV show meta data
        let mut show_ids = Vec::new();
        for show in self.user_data.subscribed_shows() {
            show_ids.push(show.id);
        }

        if show_ids.is_empty() {
            return Ok(());
        }

        let shows = self.api.get_shows(&show_ids)?;

        if self.verbose {
            println!();
        }

        // Update user data
        show_ids.clear();
        for show in shows {
            let id = show.id;
            if self.user_data.update_show(show) || force {
                show_ids.push(id);
            }
        }

        // Get episode data
        let mut episodes = self.api.get_episodes(&show_ids)?;

        if self.verbose {
            println!();
        }

        // Remove all episodes that haven't aired yet
        episodes.retain(|episode| match episode.airstamp {
            Some(airstamp) => Utc::now() >= airstamp,
            None => false,
        });

        // Remove all episodes that have already been watched
        {
            let mut index = 0;
            let mut current_show = 0;
            let subscribed_shows = self.user_data.subscribed_shows();
            episodes.retain(|episode| {
                if episode.show_id != current_show {
                    index = match subscribed_shows
                        .iter()
                        .position(|show| show.id == episode.show_id)
                    {
                        Some(index) => index,
                        None => return false,
                    };
                    current_show = episode.show_id;
                }

                let last_watched = subscribed_shows[index].last_watched_episode;
                if episode.season == last_watched.0 {
                    episode.number > last_watched.1
                } else {
                    episode.season > last_watched.0
                }
            });
        }

        // Update user data
        // TODO: maybe store both id and (season, number) in last_watched_episode field?
        //       This way, one could detect if episode number for given id ever changes..
        episodes.retain(|episode| !self.user_data.update_episode(episode));

        // Add new episodes
        if !episodes.is_empty() {
            {
                let mut show_names: HashMap<usize, &str> = HashMap::new();
                let shows = self.user_data.subscribed_shows();
                for show in shows {
                    show_names.insert(show.id, &show.name);
                }

                episodes.sort_by(|a, b| match (a.airstamp, b.airstamp) {
                    (Some(date_a), Some(date_b)) => date_a.cmp(&date_b),
                    (Some(_), None) => Ordering::Greater,
                    (None, Some(_)) => Ordering::Less,
                    (None, None) => b.cmp(a),
                });

                println!("New episodes:");
                println!();

                App::print_episode_list_as_table(
                    &episodes,
                    &HorizontalSeparator::Week,
                    Some(&show_names),
                );
                println!();
            }

            self.user_data.add_episodes(episodes);
        }

        self.user_data.store()?;

        Ok(())
    }
}
