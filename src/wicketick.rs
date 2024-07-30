use std::sync::Arc;
use std::time;

use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use crate::cricinfo;
use crate::errors::Error;

#[derive(Clone)]
pub enum Source {
    Cricinfo { match_id: String },
}

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
    pub async fn refresh(&mut self) -> Result<(), Error> {
        match self.source.clone() {
            Source::Cricinfo { match_id } => {
                self.last_refresh = Some(time::Instant::now());
                self.summary = Some(cricinfo::get_match_summary(match_id).await?);
                Ok(())
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
