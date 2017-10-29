use tvmaze_api::Show;

#[derive(Debug)]
pub struct UserData {
    version: u32,
    subscribed_shows: Vec<Show>
}

impl UserData {
    fn new() -> Self {
        Self {
            version: 1,
            subscribed_shows: Vec::new(),
        }
    }

    pub fn load() -> Self {
        UserData::new()
    }

    pub fn store(&self) {
        println!("{:#?}", self);
    }

    #[allow(dead_code)]
    fn version(&self) -> u32 {
        self.version
    }

    pub fn add_show(&mut self, show: Show) {
        if !self.subscribed_shows.contains(&show) {
            self.subscribed_shows.push(show);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tvmaze_api::{Network, Status, Schedule, Day};

    fn star_trek_discovery() -> Show {
        Show {
            id: 7480,
            name: "Star Trek: Discovery".to_string(),
            language: "English".to_string(),
            network: None,
            web_channel: Some(
                Network {
                    id: 107,
                    name: "CBS All Access".to_string()
                }
            ),
            status: Status::Running,
            runtime: Some(60),
            schedule: Schedule {
                days: vec!(Day::Sunday)
            }
        }
    }

    fn the_orville() -> Show {
        Show {
            id: 20263,
            name: "The Orville".to_string(),
            language: "English".to_string(),
            network: Some(
                Network {
                    id: 4,
                    name: "FOX".to_string()
                }
            ),
            web_channel: None,
            status: Status::Running,
            runtime: Some(60),
            schedule: Schedule {
                days: vec!(Day::Thursday)
            }
        }
    }

    #[test]
    fn version() {
        let user_data = UserData::new();
        assert_eq!(1, user_data.version());
    }

    #[test]
    fn add_new_show() {
        let mut user_data = UserData::new();
        assert!(user_data.subscribed_shows.is_empty());

        user_data.add_show(star_trek_discovery());
        assert!(user_data.subscribed_shows.iter().find(|&show| show.id == 7480).is_some());
    }

    #[test]
    fn do_not_add_already_subscribed_show() {
        let mut user_data = UserData::new();
        user_data.add_show(star_trek_discovery());
        user_data.add_show(the_orville());

        assert!(user_data.subscribed_shows.iter().find(|&show| show.id == 7480).is_some());
        assert!(user_data.subscribed_shows.iter().find(|&show| show.id == 20263).is_some());
        assert_eq!(2, user_data.subscribed_shows.len());

        user_data.add_show(the_orville());
        assert_eq!(2, user_data.subscribed_shows.len());
    }
}