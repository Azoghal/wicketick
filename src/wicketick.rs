use std::time;

use crate::cricinfo;
use crate::errors::Error;

#[derive(Clone)]
pub enum Source {
    Cricinfo { match_id: String },
}

pub struct WickeTick {
    pub source: Source,
    pub summary: Option<SimpleSummary>,
    pub last_refresh: Option<time::Instant>,
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
pub struct SimpleSummary {
    pub current_innings: Innings,
}

impl SimpleSummary {
    pub fn display(&self) -> String {
        return self.current_innings.display();
    }
}

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
