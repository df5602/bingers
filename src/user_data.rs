use std::cmp::Ordering;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::PathBuf;

use app_dirs::{get_data_root, AppDataType};

use errors::*;
use tvmaze_api::{Episode, Show, Status};

const VERSION: u32 = 1;

type EpisodeNumber = (usize, usize);

fn episode_is_greater_than(episode: &Episode, episode_number: EpisodeNumber) -> bool {
    if episode.season == episode_number.0 {
        episode.number > episode_number.1
    } else {
        episode.season > episode_number.0
    }
}

fn episode_is_less_than(episode: &Episode, episode_number: EpisodeNumber) -> bool {
    if episode.season == episode_number.0 {
        episode.number < episode_number.1
    } else {
        episode.season < episode_number.0
    }
}

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
            path,
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
                file.read_to_string(&mut file_content)
                    .chain_err(|| format!("Unable to read user data from {:?}", user_data_file))?;

                // Detect version
                let detect_version: DetectVersion = ::serde_json::from_str(&file_content)
                    .chain_err(|| format!("Unable to parse version from {:?}", user_data_file))?;

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
        fs::create_dir_all(&self.path)
            .chain_err(|| format!("Unable to create user data directory {:?}", self.path))?;

        // Create temporary file and serialize user data into it
        let mut tmp_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&user_data_tmp)
            .chain_err(|| format!("Unable to open {:?}", user_data_tmp))?;

        let json =
            ::serde_json::to_string(&self.data).chain_err(|| "Unable to serialize user data.")?;

        tmp_file
            .write_all(json.as_bytes())
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

    pub fn subscribed_shows(&self) -> &Vec<Show> {
        &self.data.subscribed_shows
    }

    pub fn subscribed_shows_by_most_recent(&self) -> Vec<&Show> {
        let mut subscribed_shows = Vec::new();

        for show in &self.data.subscribed_shows {
            subscribed_shows.push(show);
        }

        // TODO: sort by date of most recent episode
        subscribed_shows.sort_by(|a, b| match (&a.status, &b.status) {
            (&Status::Running, &Status::Running) => b.last_updated.cmp(&a.last_updated),
            (&Status::Running, _) => Ordering::Less,
            (_, &Status::Running) => Ordering::Greater,
            (&Status::ToBeDetermined, &Status::ToBeDetermined) => {
                b.last_updated.cmp(&a.last_updated)
            }
            (&Status::ToBeDetermined, _) => Ordering::Less,
            (_, &Status::ToBeDetermined) => Ordering::Greater,
            (_, _) => b.last_updated.cmp(&a.last_updated),
        });

        subscribed_shows
    }

    pub fn unwatched_episodes(&self) -> &Vec<Episode> {
        &self.data.unwatched_episodes
    }

    pub fn unwatched_episodes_oldest_first(&self) -> Vec<&Episode> {
        let mut unwatched_episodes = Vec::new();

        for episode in &self.data.unwatched_episodes {
            unwatched_episodes.push(episode);
        }

        unwatched_episodes.sort_by(|a, b| match (a.airstamp, b.airstamp) {
            (Some(date_a), Some(date_b)) => date_a.cmp(&date_b),
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            (None, None) => b.cmp(a),
        });

        unwatched_episodes
    }

    pub fn add_show(&mut self, show: Show) {
        if !self.data.subscribed_shows.contains(&show) {
            self.data.subscribed_shows.push(show);

            self.data.subscribed_shows.sort();
        }
    }

    pub fn add_episodes(&mut self, episodes: Vec<Episode>) {
        let mut episode_added = false;
        for episode in episodes {
            if !self.data.unwatched_episodes.contains(&episode) {
                self.data.unwatched_episodes.push(episode);
                episode_added = true;
            }
        }

        if episode_added {
            self.data.unwatched_episodes.sort();
        }
    }

    pub fn remove_episodes(&mut self, show: &Show) {
        self.data
            .unwatched_episodes
            .retain(|episode| episode.show_id != show.id);
    }

    pub fn remove_show(&mut self, show: &Show) {
        self.data
            .subscribed_shows
            .retain(|subscribed_show| subscribed_show != show);
    }

    /// Mark episode of given show as watched.
    ///
    /// If neither season nor episode are specified, will mark the next unwatched episode
    /// as watched.
    ///
    /// If only season is specified, will mark the whole season as watched.
    ///
    /// If both season and episode are specified, will mark the exact episode as watched.
    ///
    /// Returns episode number of last episode that was marked as watched.
    pub fn mark_as_watched(
        &mut self,
        show_id: usize,
        season: Option<usize>,
        episode: Option<usize>,
    ) -> Option<(usize, usize)> {
        // Mark episode(s) as watched
        let last_marked = match (season, episode) {
            (Some(season), None) => self.mark_season_as_watched(show_id, season),
            (Some(season), Some(episode)) => self.mark_episode_as_watched(show_id, season, episode),
            (None, None) => self.mark_next_episode_as_watched(show_id),
            (None, Some(_)) => None,
        };

        if let Some(last_marked) = last_marked {
            let mut gap = false;
            let mut last_watched = (0, 0);
            let mut show_index = None;

            // Determine last watched episode (or rather the episode before the previously
            // first unwatched episode)
            for (i, show) in self.data
                .subscribed_shows
                .iter()
                .enumerate()
                .filter(|&(_, show)| show.id == show_id)
            {
                last_watched = show.last_watched_episode;
                show_index = Some(i);
            }

            // Determine if there are unwatched episodes between the last watched episode
            // and the episodes that were now marked as watched.
            for _ in self.data
                .unwatched_episodes
                .iter()
                .filter(|episode| episode.show_id == show_id && !episode.watched)
                .filter(|episode| episode_is_greater_than(episode, last_watched))
                .filter(|episode| episode_is_less_than(episode, last_marked))
            {
                gap = true;
            }

            // Remove all watched episodes and update last watched episode.
            //
            // Don't remove watched episodes that are separated by the last_watched pointer with
            // a gap of unwatched episodes.
            //
            // Cleans up watched episodes if gap is eliminated.
            if !gap {
                let mut last_watched = last_watched;
                let mut stop = false;

                // This is slightly dirty because it depends on the internal implementation
                // of retain() (i.e. that the vector is iterated over in order from start to end).
                // Tests should catch it, if that implementation ever should change...
                self.data.unwatched_episodes.retain(|episode| {
                    if episode.show_id == show_id && episode_is_greater_than(episode, last_watched)
                    {
                        // If the episode is marked as watched and we haven't yet hit a gap...
                        if episode.watched && !stop {
                            // ... update last_watched pointer and remove episode
                            last_watched = (episode.season, episode.number);
                            false
                        } else {
                            // We hit a gap. Retain all following episodes.
                            stop = true;
                            true
                        }
                    } else {
                        // Keep episodes of other shows
                        true
                    }
                });

                if let Some(index) = show_index {
                    self.data.subscribed_shows[index].last_watched_episode = last_watched;
                }
            }
        }

        last_marked
    }

    #[allow(unknown_lints)]
    #[allow(never_loop)]
    fn mark_next_episode_as_watched(&mut self, show_id: usize) -> Option<(usize, usize)> {
        let mut marked = None;

        for episode in self.data
            .unwatched_episodes
            .iter_mut()
            .filter(|episode| episode.show_id == show_id && !episode.watched)
        {
            episode.watched = true;
            marked = Some((episode.season, episode.number));
            break;
        }

        marked
    }

    fn mark_episode_as_watched(
        &mut self,
        show_id: usize,
        season: usize,
        number: usize,
    ) -> Option<(usize, usize)> {
        let mut marked = None;

        for episode in self.data.unwatched_episodes.iter_mut().filter(|episode| {
            episode.show_id == show_id
                && episode.season == season
                && episode.number == number
                && !episode.watched
        }) {
            episode.watched = true;
            marked = Some((episode.season, episode.number));
        }

        marked
    }

    fn mark_season_as_watched(&mut self, show_id: usize, season: usize) -> Option<(usize, usize)> {
        let mut marked = None;

        for episode in self.data.unwatched_episodes.iter_mut().filter(|episode| {
            episode.show_id == show_id && episode.season == season && !episode.watched
        }) {
            episode.watched = true;
            marked = Some((episode.season, episode.number));
        }

        marked
    }

    /// Updates the metadata of a show with the one provided.
    /// Returns whether last_updated field has been updated.
    pub fn update_show(&mut self, show: Show) -> bool {
        // Find show in user data
        let subscribed_shows = &mut self.data.subscribed_shows;
        let index = match subscribed_shows.iter().position(|elem| elem.id == show.id) {
            Some(index) => index,
            None => return false,
        };
        let stored_show = &mut subscribed_shows[index];

        // Update name
        if stored_show.name != show.name {
            println!("\"{}\" changed to \"{}\"", stored_show.name, show.name);
            stored_show.name = show.name;
        }

        // Update language
        stored_show.language = show.language;

        // Update network
        stored_show.network = show.network;

        // Update web channel
        stored_show.web_channel = show.web_channel;

        // Update status
        if stored_show.status != show.status {
            println!(
                "{}: Changed from {} to {}",
                stored_show.name, stored_show.status, show.status
            );
            stored_show.status = show.status;
        }

        // Update runtime
        stored_show.runtime = show.runtime;

        // Update schedule
        stored_show.schedule = show.schedule;

        // Last updated
        // TODO: also store previous_episode field and see if that has changed
        if stored_show.last_updated != show.last_updated {
            stored_show.last_updated = show.last_updated;
            return true;
        }

        false
    }

    /// Updates the meta data of an episode with the one provided.
    /// Returns true if episode has been found, false otherwise.
    pub fn update_episode(&mut self, episode: &Episode) -> bool {
        // Find episode in user data
        let unwatched_episodes = &mut self.data.unwatched_episodes;
        let index = match unwatched_episodes
            .iter()
            .position(|elem| elem.episode_id == episode.episode_id)
        {
            Some(index) => index,
            None => return false,
        };
        let stored_episode = &mut unwatched_episodes[index];

        // Update name
        if stored_episode.name != episode.name {
            println!(
                "\"{}\" changed to \"{}\"",
                stored_episode.name, episode.name
            );
            stored_episode.name = episode.name.clone();
        }

        // Update season / number
        if stored_episode.season != episode.season || stored_episode.number != episode.number {
            println!(
                "{}: Changed from being season {} episode {} to season {} episode {}",
                stored_episode.name,
                stored_episode.season,
                stored_episode.number,
                episode.season,
                episode.number
            );
            stored_episode.season = episode.season;
            stored_episode.number = episode.number;
        }

        // Update airstamp
        stored_episode.airstamp = episode.airstamp;

        // Update runtime
        stored_episode.runtime = episode.runtime;

        true
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

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
            last_updated: 0,
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
            last_updated: 0,
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
            airstamp: Some(Utc.ymd(2017, 9, 10).and_hms(0, 0, 0)),
            runtime: 60,
            watched: false,
        }
    }

    fn the_orville_ep2() -> Episode {
        Episode {
            episode_id: 1201556,
            show_id: 20263,
            name: "Command Performance".to_string(),
            season: 1,
            number: 2,
            airstamp: Some(Utc.ymd(2017, 9, 17).and_hms(0, 0, 0)),
            runtime: 60,
            watched: false,
        }
    }

    fn the_orville_ep3() -> Episode {
        Episode {
            episode_id: 1201557,
            show_id: 20263,
            name: "About a Girl".to_string(),
            season: 1,
            number: 3,
            airstamp: Some(Utc.ymd(2017, 9, 22).and_hms(1, 0, 0)),
            runtime: 60,
            watched: false,
        }
    }

    fn the_orville_ep4() -> Episode {
        Episode {
            episode_id: 1201558,
            show_id: 20263,
            name: "If the Stars Should Appear".to_string(),
            season: 1,
            number: 4,
            airstamp: Some(Utc.ymd(2017, 9, 29).and_hms(1, 0, 0)),
            runtime: 60,
            watched: false,
        }
    }

    fn the_orville_season2_ep1() -> Episode {
        Episode {
            episode_id: 15151515,
            show_id: 20263,
            name: "Future Episode".to_string(),
            season: 2,
            number: 1,
            airstamp: Some(Utc.ymd(2018, 3, 15).and_hms(0, 0, 0)),
            runtime: 60,
            watched: false,
        }
    }

    fn star_trek_discovery_ep1() -> Episode {
        Episode {
            episode_id: 892064,
            show_id: 7480,
            name: "The Vulcan Hello".to_string(),
            season: 1,
            number: 1,
            airstamp: Some(Utc.ymd(2017, 9, 25).and_hms(0, 30, 0)),
            runtime: 60,
            watched: false,
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

    #[test]
    fn remove_episodes_of_a_given_show() {
        let mut user_data = load_dev_user_data();
        user_data.add_show(the_orville());
        user_data.add_show(star_trek_discovery());
        user_data.add_episodes(vec![
            the_orville_ep1(),
            the_orville_ep2(),
            star_trek_discovery_ep1(),
        ]);

        assert!(
            user_data
                .data
                .unwatched_episodes
                .contains(&the_orville_ep1())
        );
        assert!(
            user_data
                .data
                .unwatched_episodes
                .contains(&the_orville_ep2())
        );
        assert!(
            user_data
                .data
                .unwatched_episodes
                .contains(&star_trek_discovery_ep1())
        );
        assert_eq!(3, user_data.data.unwatched_episodes.len());

        user_data.remove_episodes(&the_orville());

        assert!(
            user_data
                .data
                .unwatched_episodes
                .contains(&star_trek_discovery_ep1())
        );
        assert_eq!(1, user_data.data.unwatched_episodes.len());
    }

    #[test]
    fn mark_next_episode_as_unwatched() {
        let mut user_data = load_dev_user_data();
        user_data.add_show(the_orville());
        user_data.add_show(star_trek_discovery());
        user_data.add_episodes(vec![
            the_orville_ep1(),
            the_orville_ep2(),
            star_trek_discovery_ep1(),
        ]);
        assert_eq!(3, user_data.data.unwatched_episodes.len());
        assert_eq!(
            (0, 0),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert_eq!(Some((1, 1)), user_data.mark_as_watched(20263, None, None));

        assert!(
            user_data
                .data
                .unwatched_episodes
                .contains(&the_orville_ep2())
        );
        assert_eq!(2, user_data.data.unwatched_episodes.len());

        assert_eq!(
            (1, 1),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert!(!user_data.data.unwatched_episodes[0].watched);
        assert!(!user_data.data.unwatched_episodes[1].watched);

        assert_eq!(Some((1, 2)), user_data.mark_as_watched(20263, None, None));

        assert!(
            user_data
                .data
                .unwatched_episodes
                .contains(&star_trek_discovery_ep1())
        );
        assert_eq!(1, user_data.data.unwatched_episodes.len());

        assert_eq!(
            (1, 2),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert!(!user_data.data.unwatched_episodes[0].watched);

        assert_eq!(None, user_data.mark_as_watched(20263, None, None));

        assert!(!user_data.data.unwatched_episodes[0].watched);
        assert_eq!(
            (1, 2),
            user_data.data.subscribed_shows[1].last_watched_episode
        );
    }

    #[test]
    fn mark_season_as_unwatched() {
        let mut user_data = load_dev_user_data();
        user_data.add_show(the_orville());
        user_data.add_show(star_trek_discovery());
        user_data.add_episodes(vec![
            the_orville_ep1(),
            the_orville_ep2(),
            star_trek_discovery_ep1(),
        ]);
        assert_eq!(3, user_data.data.unwatched_episodes.len());
        assert_eq!(
            (0, 0),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert_eq!(
            Some((1, 2)),
            user_data.mark_as_watched(20263, Some(1), None)
        );

        assert!(
            user_data
                .data
                .unwatched_episodes
                .contains(&star_trek_discovery_ep1())
        );
        assert_eq!(1, user_data.data.unwatched_episodes.len());

        assert!(!user_data.data.unwatched_episodes[0].watched);
        assert_eq!(
            (1, 2),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert_eq!(None, user_data.mark_as_watched(20263, Some(1), None));

        assert!(
            user_data
                .data
                .unwatched_episodes
                .contains(&star_trek_discovery_ep1())
        );
        assert_eq!(1, user_data.data.unwatched_episodes.len());

        assert!(!user_data.data.unwatched_episodes[0].watched);
        assert_eq!(
            (1, 2),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert_eq!(None, user_data.mark_as_watched(20263, Some(2), None));

        assert!(!user_data.data.unwatched_episodes[0].watched);
        assert_eq!(1, user_data.data.unwatched_episodes.len());
        assert_eq!(
            (1, 2),
            user_data.data.subscribed_shows[1].last_watched_episode
        );
    }

    #[test]
    fn mark_season_as_unwatched_with_gap() {
        let mut user_data = load_dev_user_data();
        user_data.add_show(the_orville());
        user_data.add_show(star_trek_discovery());
        user_data.add_episodes(vec![
            the_orville_ep1(),
            the_orville_ep2(),
            the_orville_season2_ep1(),
            star_trek_discovery_ep1(),
        ]);
        assert_eq!(4, user_data.data.unwatched_episodes.len());
        assert_eq!(
            (0, 0),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert_eq!(
            Some((2, 1)),
            user_data.mark_as_watched(20263, Some(2), None)
        );

        assert!(!user_data.data.unwatched_episodes[0].watched);
        assert!(!user_data.data.unwatched_episodes[1].watched);
        assert!(!user_data.data.unwatched_episodes[2].watched);
        assert!(user_data.data.unwatched_episodes[3].watched);
        assert_eq!(
            (0, 0),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert_eq!(None, user_data.mark_as_watched(20263, Some(2), None));

        assert!(!user_data.data.unwatched_episodes[0].watched);
        assert!(!user_data.data.unwatched_episodes[1].watched);
        assert!(!user_data.data.unwatched_episodes[2].watched);
        assert!(user_data.data.unwatched_episodes[3].watched);
        assert_eq!(
            (0, 0),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert_eq!(None, user_data.mark_as_watched(20263, Some(3), None));

        assert!(!user_data.data.unwatched_episodes[0].watched);
        assert!(!user_data.data.unwatched_episodes[1].watched);
        assert!(!user_data.data.unwatched_episodes[2].watched);
        assert!(user_data.data.unwatched_episodes[3].watched);
        assert_eq!(
            (0, 0),
            user_data.data.subscribed_shows[1].last_watched_episode
        );
    }

    #[test]
    fn mark_episode_as_unwatched() {
        let mut user_data = load_dev_user_data();
        user_data.add_show(the_orville());
        user_data.add_show(star_trek_discovery());
        user_data.add_episodes(vec![
            the_orville_ep1(),
            the_orville_ep2(),
            star_trek_discovery_ep1(),
        ]);
        assert_eq!(3, user_data.data.unwatched_episodes.len());
        assert_eq!(
            (0, 0),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert_eq!(
            Some((1, 1)),
            user_data.mark_as_watched(20263, Some(1), Some(1))
        );

        assert!(
            user_data
                .data
                .unwatched_episodes
                .contains(&the_orville_ep2())
        );
        assert_eq!(2, user_data.data.unwatched_episodes.len());

        assert!(!user_data.data.unwatched_episodes[0].watched);
        assert!(!user_data.data.unwatched_episodes[1].watched);
        assert_eq!(
            (1, 1),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert_eq!(
            Some((1, 2)),
            user_data.mark_as_watched(20263, Some(1), Some(2))
        );

        assert!(
            user_data
                .data
                .unwatched_episodes
                .contains(&star_trek_discovery_ep1())
        );
        assert_eq!(1, user_data.data.unwatched_episodes.len());

        assert!(!user_data.data.unwatched_episodes[0].watched);
        assert_eq!(
            (1, 2),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert_eq!(None, user_data.mark_as_watched(20263, Some(1), Some(2)));

        assert!(!user_data.data.unwatched_episodes[0].watched);
        assert_eq!(1, user_data.data.unwatched_episodes.len());
        assert_eq!(
            (1, 2),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert_eq!(None, user_data.mark_as_watched(20263, Some(1), Some(3)));

        assert!(!user_data.data.unwatched_episodes[0].watched);
        assert_eq!(1, user_data.data.unwatched_episodes.len());
        assert_eq!(
            (1, 2),
            user_data.data.subscribed_shows[1].last_watched_episode
        );
    }

    #[test]
    fn mark_episode_as_unwatched_with_gap() {
        let mut user_data = load_dev_user_data();
        user_data.add_show(the_orville());
        user_data.add_show(star_trek_discovery());
        user_data.add_episodes(vec![
            the_orville_ep1(),
            the_orville_ep2(),
            star_trek_discovery_ep1(),
        ]);
        assert_eq!(3, user_data.data.unwatched_episodes.len());
        assert_eq!(
            (0, 0),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert_eq!(
            Some((1, 2)),
            user_data.mark_as_watched(20263, Some(1), Some(2))
        );

        assert!(!user_data.data.unwatched_episodes[0].watched);
        assert!(!user_data.data.unwatched_episodes[1].watched);
        assert!(user_data.data.unwatched_episodes[2].watched);
        assert_eq!(
            (0, 0),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert_eq!(None, user_data.mark_as_watched(20263, Some(1), Some(2)));

        assert!(!user_data.data.unwatched_episodes[0].watched);
        assert!(!user_data.data.unwatched_episodes[1].watched);
        assert!(user_data.data.unwatched_episodes[2].watched);
        assert_eq!(
            (0, 0),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert_eq!(None, user_data.mark_as_watched(20263, Some(1), Some(3)));

        assert!(!user_data.data.unwatched_episodes[0].watched);
        assert!(!user_data.data.unwatched_episodes[1].watched);
        assert!(user_data.data.unwatched_episodes[2].watched);
        assert_eq!(
            (0, 0),
            user_data.data.subscribed_shows[1].last_watched_episode
        );
    }

    #[test]
    fn keep_watched_episodes_after_gap() {
        let mut user_data = load_dev_user_data();
        user_data.add_show(the_orville());
        user_data.add_show(star_trek_discovery());
        user_data.add_episodes(vec![
            the_orville_ep1(),
            the_orville_ep2(),
            the_orville_ep3(),
            the_orville_ep4(),
            star_trek_discovery_ep1(),
        ]);
        assert_eq!(5, user_data.data.unwatched_episodes.len());
        assert_eq!(
            (0, 0),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert_eq!(Some((1, 1)), user_data.mark_as_watched(20263, None, None));
        assert_eq!(
            (1, 1),
            user_data.data.subscribed_shows[1].last_watched_episode
        );
        assert_eq!(4, user_data.data.unwatched_episodes.len());

        assert_eq!(
            Some((1, 4)),
            user_data.mark_as_watched(20263, Some(1), Some(4))
        );
        assert_eq!(
            (1, 1),
            user_data.data.subscribed_shows[1].last_watched_episode
        );
        assert_eq!(4, user_data.data.unwatched_episodes.len());

        assert_eq!(
            Some((1, 2)),
            user_data.mark_as_watched(20263, Some(1), Some(2))
        );
        assert_eq!(
            (1, 2),
            user_data.data.subscribed_shows[1].last_watched_episode
        );
        assert!(
            user_data
                .data
                .unwatched_episodes
                .contains(&the_orville_ep3())
        );
        assert!(
            user_data
                .data
                .unwatched_episodes
                .contains(&the_orville_ep4())
        );
        assert!(!user_data.data.unwatched_episodes[1].watched);
        assert!(user_data.data.unwatched_episodes[2].watched);
        assert_eq!(3, user_data.data.unwatched_episodes.len());
    }

    #[test]
    fn remove_other_watched_episodes_if_gap_is_eliminated() {
        let mut user_data = load_dev_user_data();
        user_data.add_show(the_orville());
        user_data.add_show(star_trek_discovery());
        user_data.add_episodes(vec![
            the_orville_ep1(),
            the_orville_ep2(),
            the_orville_ep3(),
            star_trek_discovery_ep1(),
        ]);
        assert_eq!(4, user_data.data.unwatched_episodes.len());
        assert_eq!(
            (0, 0),
            user_data.data.subscribed_shows[1].last_watched_episode
        );

        assert_eq!(Some((1, 1)), user_data.mark_as_watched(20263, None, None));
        assert_eq!(
            (1, 1),
            user_data.data.subscribed_shows[1].last_watched_episode
        );
        assert_eq!(3, user_data.data.unwatched_episodes.len());

        assert_eq!(
            Some((1, 3)),
            user_data.mark_as_watched(20263, Some(1), Some(3))
        );
        assert_eq!(
            (1, 1),
            user_data.data.subscribed_shows[1].last_watched_episode
        );
        assert_eq!(3, user_data.data.unwatched_episodes.len());
        assert!(!user_data.data.unwatched_episodes[1].watched);
        assert!(user_data.data.unwatched_episodes[2].watched);

        assert_eq!(
            Some((1, 2)),
            user_data.mark_as_watched(20263, Some(1), Some(2))
        );
        assert_eq!(
            (1, 3),
            user_data.data.subscribed_shows[1].last_watched_episode
        );
        assert_eq!(1, user_data.data.unwatched_episodes.len());
        assert!(
            user_data
                .data
                .unwatched_episodes
                .contains(&star_trek_discovery_ep1())
        );
    }
}
