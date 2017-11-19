use std::path::PathBuf;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::cmp::Ordering;

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

    #[allow(unknown_lints)]
    #[allow(match_same_arms)] // Clippy issue #860
    pub fn subscribed_shows_by_most_recent(&self) -> Vec<&Show> {
        let mut subscribed_shows = Vec::new();

        for show in &self.data.subscribed_shows {
            subscribed_shows.push(show);
        }

        // TODO: sort by date of most recent episode
        subscribed_shows.sort_by(|a, b| match (&a.status, &b.status) {
            (&Status::Running, &Status::Running) => a.cmp(b),
            (&Status::Running, _) => Ordering::Less,
            (_, &Status::Running) => Ordering::Greater,
            (_, _) => a.cmp(b),
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

            // TODO: handle more complicated case where there was a gap
            // from a previous "mark as watched" command
            //
            // Remove all watched episodes and update last watched episode
            if !gap {
                self.data
                    .unwatched_episodes
                    .retain(|episode| episode.show_id != show_id || !episode.watched);

                if let Some(index) = show_index {
                    self.data.subscribed_shows[index].last_watched_episode = last_marked;
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
            episode.show_id == show_id && episode.season == season && episode.number == number
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
}
