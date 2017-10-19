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
            let network_name = match result.show.network {
                Some(ref network) => network.name.clone(),
                None => match result.show.web_channel {
                    Some(ref channel) => channel.name.clone(),
                    None => "Unknown".to_string(),
                },
            };

            let scheduled_days = if !result.show.schedule.days.is_empty() {
                if result.show.status == Status::Running {
                    format!("{:?}s on ", result.show.schedule.days[0])
                } else {
                    "".to_string()
                }
            } else {
                "".to_string()
            };

            let runtime = match result.show.runtime {
                Some(runtime) => runtime,
                None => 0,
            };

            let status = match result.show.status {
                Status::Ended => " (Ended)",
                Status::ToBeDetermined => " (TBD)",
                _ => "",
            };

            println!(
                "[{}] {} ({}{}{}, {}')",
                i,
                result.show.name,
                scheduled_days,
                network_name,
                status,
                runtime
            );
        }

        Ok(())
    }
}
