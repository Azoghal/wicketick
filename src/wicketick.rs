use std::sync::Arc;
use std::{fmt, time};

use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use crate::cricinfo;
use crate::errors::Error;

#[derive(Clone)]
pub enum Source {
    Cricinfo { match_id: Option<String> },
    _SomeApi { base_url: String, api_token: String },
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Cricinfo { match_id } => write!(f, "CricInfo(match_id:{:?})", match_id),
            Source::_SomeApi {
                base_url,
                api_token,
            } => write!(f, "_SomeApi"),
        }
    }
}

// TODO really we need a trait for all the things we need to do with a
// impl Source {
//     fn should_poll(self) -> bool {
//         match cricinfo {}
//     }
// }

pub static DEFAULT_POLL_INTERVAL: time::Duration = time::Duration::from_secs(30);

#[derive(Clone)]
pub struct WickeTick {
    pub source: Source,
    pub summary: Option<SimpleSummary>,
    pub last_refresh: Option<time::Instant>,
    pub poll_interval: Option<time::Duration>,
}

pub async fn poll_wicketick(wicketick: Arc<Mutex<WickeTick>>, interval: Duration) {
    loop {
        {
            let mut locked_wicketick = wicketick.lock().await;
            let res = locked_wicketick.refresh().await;
            if let Err(err) = res {
                // TODO proper logging, or somewhere to display it in the canvas
                println!("failed to poll: {}", err)
            }
        }
        sleep(interval).await;
    }
}

impl WickeTick {
    pub fn new(source: Source, poll_interval: Option<time::Duration>) -> Self {
        let poll_t = match poll_interval {
            Some(t) => t,
            None => DEFAULT_POLL_INTERVAL,
        };
        return Self {
            source,
            summary: None,
            last_refresh: None,
            poll_interval: Some(poll_t),
        };
    }

    pub async fn refresh(&mut self) -> Result<(), Error> {
        match self.source.clone() {
            Source::Cricinfo {
                match_id: Some(m_id),
            } => {
                self.last_refresh = Some(time::Instant::now());
                self.summary = Some(cricinfo::get_match_summary(m_id).await?);
                Ok(())
            }
            Source::Cricinfo { match_id: None } => {
                // Nothing to refresh
                Ok(())
            }
            _ => Err(Error::Todo("not implemented".to_string())),
        }
    }

    pub async fn refetch(self) -> Result<SimpleSummary, Error> {
        match self.source.clone() {
            Source::Cricinfo {
                match_id: Some(m_id),
            } => cricinfo::get_match_summary(m_id).await,
            Source::Cricinfo { match_id: None } => {
                // Nothing to refresh
                Err(Error::Todo("no match id".to_string()))
            }
            _ => Err(Error::Todo("not implemented".to_string())),
        }
    }
}

// Simple summary just stores one innings, for now
#[derive(Clone)]
pub struct SimpleSummary {
    pub current_innings: Innings,
}

impl SimpleSummary {
    pub fn display(&self) -> String {
        return self.current_innings.display();
    }
}

#[derive(Clone)]
pub struct Innings {
    pub runs: i32,
    pub wickets: i32,
    pub overs: String,
}

impl Innings {
    pub fn display(&self) -> String {
        let runs = self.runs;
        let wickets = self.wickets;
        let overs = &self.overs;
        let text = format!("{}-{} {}", runs, wickets, overs);
        return text;
    }
}
