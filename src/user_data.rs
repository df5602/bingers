use std::path::PathBuf;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};

use app_dirs::{get_data_root, AppDataType};

#[allow(unused_imports)]
use chrono::{TimeZone, Utc};

use errors::*;
use tvmaze_api::{Episode, Show};

const VERSION: u32 = 1;

#[derive(Deserialize)]
struct DetectVersion {
    version: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct UserDataV1 {
    version: u32,
    subscribed_shows: Vec<Show>,
    unwatched_episodes: Vec<Episode>,
}

#[derive(Debug)]
pub struct UserData {
    path: PathBuf,
    data: UserDataV1,
}

impl UserData {
    fn new(path: PathBuf) -> Self {
        Self {
            path: path,
            data: UserDataV1 {
                version: 1,
                subscribed_shows: Vec::new(),
                unwatched_episodes: Vec::new(),
            },
        }
    }

    pub fn load() -> Result<Self> {
        let mut user_data_path = get_data_root(AppDataType::UserData)
            .chain_err(|| "Unable to determine user data location.")?;
        user_data_path.push("bingers");

        let mut user_data_file = user_data_path.clone();
        user_data_file.push("user_data.json");

        match File::open(&user_data_file) {
            Ok(mut file) => {
                // Read user data from file
                let mut file_content = String::new();
                file.read_to_string(&mut file_content).chain_err(|| {
                    format!("Unable to read user data from {:?}", user_data_file)
                })?;

                // Detect version
                let detect_version: DetectVersion = ::serde_json::from_str(&file_content)
                    .chain_err(|| {
                        format!("Unable to parse version from {:?}", user_data_file)
                    })?;

                if detect_version.version > VERSION {
                    return Err(
                        ErrorKind::UserDataVersionMismatch(VERSION, detect_version.version).into(),
                    );
                }

                // Deserialize
                let mut user_data = UserData::new(user_data_path);
                user_data.data = ::serde_json::from_str(&file_content).chain_err(|| {
                    format!("Unable to deserialize user data from {:?}", user_data_file)
                })?;

                Ok(user_data)
            }
            Err(e) => {
                match e.kind() {
                    // File doesn't exist yet, so create new user data
                    io::ErrorKind::NotFound => {
                        println!("No user data found, creating new.");
                        Ok(UserData::new(user_data_path))
                    }
                    _ => Err(e.into()),
                }
            }
        }
    }

    pub fn store(&self) -> Result<()> {
        let mut user_data_tmp = self.path.clone();
        user_data_tmp.push("user_data.tmp");

        let mut user_data_json = self.path.clone();
        user_data_json.push("user_data.json");

        // Create directory (if necessary)
        fs::create_dir_all(&self.path).chain_err(|| {
            format!("Unable to create user data directory {:?}", self.path)
        })?;

        // Create temporary file and serialize user data into it
        let mut tmp_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&user_data_tmp)
            .chain_err(|| format!("Unable to open {:?}", user_data_tmp))?;

        let json =
            ::serde_json::to_string(&self.data).chain_err(|| "Unable to serialize user data.")?;

        tmp_file.write_all(json.as_bytes()).chain_err(|| {
            format!("Unable to write user data to {:?}", user_data_tmp)
        })?;

        // Move tmp file into actual user data file
        fs::rename(&user_data_tmp, &user_data_json).chain_err(|| {
            format!("Unable to move {:?} to {:?}", user_data_tmp, user_data_json)
        })?;

        Ok(())
    }

    #[allow(dead_code)]
    fn version(&self) -> u32 {
        self.data.version
    }

    pub fn subscribed_shows(&self) -> &Vec<Show> {
        &self.data.subscribed_shows
    }

    pub fn add_show(&mut self, show: Show) {
        if !self.data.subscribed_shows.contains(&show) {
            self.data.subscribed_shows.push(show);
        }
    }

    pub fn add_episodes(&mut self, episodes: Vec<Episode>) {
        for episode in episodes {
            if !self.data.unwatched_episodes.contains(&episode) {
                self.data.unwatched_episodes.push(episode);
            }
        }
    }

    pub fn remove_show(&mut self, show: &Show) {
        let position = self.data
            .subscribed_shows
            .iter()
            .position(|subscribed_show| subscribed_show == show);

        if let Some(index) = position {
            self.data.subscribed_shows.remove(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tvmaze_api::{Day, Network, Schedule, Status};

    fn star_trek_discovery() -> Show {
        Show {
            id: 7480,
            name: "Star Trek: Discovery".to_string(),
            language: Some("English".to_string()),
            network: None,
            web_channel: Some(Network {
                id: 107,
                name: "CBS All Access".to_string(),
            }),
            status: Status::Running,
            runtime: Some(60),
            schedule: Schedule {
                days: vec![Day::Sunday],
            },
            last_watched_episode: (0, 0),
        }
    }

    fn the_orville() -> Show {
        Show {
            id: 20263,
            name: "The Orville".to_string(),
            language: Some("English".to_string()),
            network: Some(Network {
                id: 4,
                name: "FOX".to_string(),
            }),
            web_channel: None,
            status: Status::Running,
            runtime: Some(60),
            schedule: Schedule {
                days: vec![Day::Thursday],
            },
            last_watched_episode: (0, 0),
        }
    }

    fn the_orville_ep1() -> Episode {
        Episode {
            episode_id: 1172410,
            show_id: 20263,
            name: "Old Wounds".to_string(),
            season: 1,
            number: 1,
            airstamp: Utc.ymd(2017, 9, 10).and_hms(0, 0, 0),
            runtime: 60,
        }
    }

    fn load_dev_user_data() -> UserData {
        let mut user_data_path = get_data_root(AppDataType::UserData).unwrap();
        user_data_path.push("bingers_dev");
        assert!(user_data_path.ends_with("bingers_dev"));

        UserData::new(user_data_path)
    }

    #[test]
    fn version() {
        let user_data = load_dev_user_data();
        assert_eq!(1, user_data.version());
    }

    #[test]
    fn add_new_show() {
        let mut user_data = load_dev_user_data();
        assert!(user_data.subscribed_shows().is_empty());

        user_data.add_show(star_trek_discovery());
        assert!(
            user_data
                .subscribed_shows()
                .contains(&star_trek_discovery())
        );
    }

    #[test]
    fn do_not_add_already_subscribed_show() {
        let mut user_data = load_dev_user_data();
        user_data.add_show(star_trek_discovery());
        user_data.add_show(the_orville());

        assert!(
            user_data
                .subscribed_shows()
                .contains(&star_trek_discovery())
        );
        assert!(user_data.subscribed_shows().contains(&the_orville()));
        assert_eq!(2, user_data.subscribed_shows().len());

        user_data.add_show(the_orville());
        assert_eq!(2, user_data.subscribed_shows().len());
    }

    #[test]
    fn remove_show() {
        let mut user_data = load_dev_user_data();
        user_data.add_show(star_trek_discovery());
        user_data.add_show(the_orville());

        assert!(
            user_data
                .subscribed_shows()
                .contains(&star_trek_discovery())
        );
        assert!(user_data.subscribed_shows().contains(&the_orville()));

        user_data.remove_show(&star_trek_discovery());

        assert!(!user_data
            .subscribed_shows()
            .contains(&star_trek_discovery()));
        assert!(user_data.subscribed_shows().contains(&the_orville()));
    }

    #[test]
    fn add_episode() {
        let mut user_data = load_dev_user_data();
        user_data.add_show(the_orville());
        user_data.add_episodes(vec![the_orville_ep1()]);

        assert!(
            user_data
                .data
                .unwatched_episodes
                .contains(&the_orville_ep1())
        );
    }

    #[test]
    fn do_not_add_episode_twice() {
        let mut user_data = load_dev_user_data();
        user_data.add_show(the_orville());
        user_data.add_episodes(vec![the_orville_ep1()]);

        assert!(
            user_data
                .data
                .unwatched_episodes
                .contains(&the_orville_ep1())
        );
        assert_eq!(1, user_data.data.unwatched_episodes.len());

        user_data.add_episodes(vec![the_orville_ep1()]);
        assert_eq!(1, user_data.data.unwatched_episodes.len());
    }
}
