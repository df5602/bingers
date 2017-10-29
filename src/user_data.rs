use std::path::PathBuf;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};

use app_dirs::{AppDataType, get_data_root};

use errors::*;
use tvmaze_api::Show;

const VERSION: u32 = 1;

#[derive(Deserialize)]
struct DetectVersion {
    version: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct UserDataV1 {
    version: u32,
    subscribed_shows: Vec<Show>
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
            }
        }
    }

    pub fn load() -> Result<Self> {
        let mut user_data_path = get_data_root(AppDataType::UserData)
                                 .chain_err(|| format!("Unable to determine user data location."))?;
        user_data_path.push("bingers");

        let mut user_data_file = user_data_path.clone();
        user_data_file.push("user_data.json");

        match File::open(&user_data_file) {
            Ok(mut file) => {
                // Read user data from file
                let mut file_content = String::new();
                file.read_to_string(&mut file_content)
                    .chain_err(|| format!("Unable to read user data from {:?}", user_data_file))?;

                // Detect version
                let detect_version: DetectVersion = ::serde_json::from_str(&file_content)
                    .chain_err(|| format!("Unable to parse version from {:?}", user_data_file))?;

                if detect_version.version > VERSION {
                    return Err(ErrorKind::UserDataVersionMismatch(VERSION, detect_version.version).into());
                }
                
                // Deserialize
                let mut user_data = UserData::new(user_data_path);
                user_data.data = ::serde_json::from_str(&file_content)
                        .chain_err(|| format!("Unable to deserialize user data from {:?}", user_data_file))?;

                Ok(user_data)
            }
            Err(e) => {
                match e.kind() {
                    // File doesn't exist yet, so create new user data
                    io::ErrorKind::NotFound => {
                        println!("No user data found, creating new.");
                        Ok(UserData::new(user_data_path))
                    },
                    _ => return Err(e.into()),
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
        fs::create_dir_all(&self.path)
            .chain_err(|| format!("Unable to create user data directory {:?}", self.path))?;

        // Create temporary file and serialize user data into it
        let mut tmp_file = OpenOptions::new()
                                        .write(true)
                                        .truncate(true)
                                        .create(true)
                                        .open(&user_data_tmp)
                                        .chain_err(|| format!("Unable to open {:?}", user_data_tmp))?;

        let json = ::serde_json::to_string(&self.data)
                                 .chain_err(|| format!("Unable to serialize user data."))?;

        tmp_file.write_all(json.as_bytes())
                .chain_err(|| format!("Unable to write user data to {:?}", user_data_tmp))?;

        // Move tmp file into actual user data file
        fs::rename(&user_data_tmp, &user_data_json)
            .chain_err(|| format!("Unable to move {:?} to {:?}", user_data_tmp, user_data_json))?;

        Ok(())
    }

    #[allow(dead_code)]
    fn version(&self) -> u32 {
        self.data.version
    }

    pub fn add_show(&mut self, show: Show) {
        if !self.data.subscribed_shows.contains(&show) {
            self.data.subscribed_shows.push(show);
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
        assert!(user_data.data.subscribed_shows.is_empty());

        user_data.add_show(star_trek_discovery());
        assert!(user_data.data.subscribed_shows.iter().find(|&show| show.id == 7480).is_some());
    }

    #[test]
    fn do_not_add_already_subscribed_show() {
        let mut user_data = load_dev_user_data();
        user_data.add_show(star_trek_discovery());
        user_data.add_show(the_orville());

        assert!(user_data.data.subscribed_shows.iter().find(|&show| show.id == 7480).is_some());
        assert!(user_data.data.subscribed_shows.iter().find(|&show| show.id == 20263).is_some());
        assert_eq!(2, user_data.data.subscribed_shows.len());

        user_data.add_show(the_orville());
        assert_eq!(2, user_data.data.subscribed_shows.len());
    }
}