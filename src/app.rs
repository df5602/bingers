use tvmaze_api::TvMazeApi;

pub struct App {
    api: TvMazeApi,
}

impl App {
    pub fn new() -> Self {
        Self {
            api: TvMazeApi::new(),
        }
    }

    /// Add show to list of followed shows.
    ///
    /// Calls web API to search for shows with the given name.
    pub fn add_show(&mut self, show: &str) {
        let search_results = self.api.search_shows(show);

        //TODO: provide unfiltered view as fallback
        for (i, result) in search_results
            .iter()
            .filter(|result| result.show.status == "Running")
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
            println!("[{}] {} ({})", i, result.show.name, network_name);
        }
    }
}
