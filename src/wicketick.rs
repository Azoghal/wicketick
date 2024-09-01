use std::fmt;
use std::str::FromStr;

use tokio::time;

use crate::errors::Error;
use crate::{cricinfo, errors};

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
                base_url: _,
                api_token: _,
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

pub static DEFAULT_POLL_INTERVAL_SECS: u64 = 30;
pub static DEFAULT_POLL_INTERVAL: time::Duration =
    time::Duration::from_secs(DEFAULT_POLL_INTERVAL_SECS);

#[derive(Clone)]
pub struct WickeTick {
    pub source: Source,
    pub summary: Option<SimpleSummary>,
    pub last_refresh: Option<time::Instant>,
    pub poll_interval: Option<time::Duration>,
}

// pub async fn poll_wicketick(wicketick: Arc<Mutex<WickeTick>>, interval: Duration) {
//     loop {
//         {
//             let mut locked_wicketick = wicketick.lock().await;
//             let res = locked_wicketick.refresh().await;
//             if let Err(err) = res {
//                 // TODO proper logging, or somewhere to display it in the canvas
//                 println!("failed to poll: {}", err)
//             }
//         }
//         sleep(interval).await;
//     }
// }

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

    pub async fn refetch(&self) -> Result<SimpleSummary, Error> {
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
    pub active_players: ActivePlayers,
    pub debug_string: String,
}

impl SimpleSummary {
    // display will just return the simplest summary.
    // display should be called on each summary field by the configurations in order
    // to get the relevant strings
    pub fn display(&self) -> String {
        if self.debug_string != "" {
            return format!("{} {}", self.current_innings.display(), self.debug_string);
        }
        self.current_innings.display()
    }

    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for SimpleSummary {
    fn default() -> Self {
        Self {
            current_innings: Innings::new(),
            active_players: ActivePlayers::default(),
            debug_string: "".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct Innings {
    pub runs: u32,
    pub wickets: u32,
    pub overs: String,
}

impl Innings {
    // displays the high level sumamry of the innings - runs, wickets, overs
    pub fn display(&self) -> String {
        let runs = self.runs;
        let wickets = self.wickets;
        let overs = &self.overs;
        let text = format!("{}-{} {}", runs, wickets, overs);
        return text;
    }

    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for Innings {
    fn default() -> Self {
        Self {
            runs: 0,
            wickets: 0,
            overs: "0".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct ActivePlayers {
    pub batter_one: Option<Batter>,
    pub batter_two: Option<Batter>,
    pub bowler_one: Option<Bowler>,
    pub bowler_two: Option<Bowler>,
}

impl Default for ActivePlayers {
    fn default() -> Self {
        Self {
            batter_one: None,
            batter_two: None,
            bowler_one: None,
            bowler_two: None,
        }
    }
}

impl ActivePlayers {
    // TODO change return type to e.g. be a tuple of the different things so they can be separated?
    pub fn display_bowlers(&self) -> String {
        let one_string = match &self.bowler_one {
            Some(bowler) => bowler.display(),
            None => "".to_string(),
        };
        let two_string = match &self.bowler_two {
            Some(bowler) => bowler.display(),
            None => "".to_string(),
        };
        format!("{} | {}", one_string, two_string)
    }

    pub fn display_batters(&self) -> String {
        let one_string = match &self.batter_one {
            Some(batter) => batter.display(),
            None => "".to_string(),
        };
        let two_string = match &self.batter_two {
            Some(batter) => batter.display(),
            None => "".to_string(),
        };
        format!("{} | {}", one_string, two_string)
    }
}

#[derive(Clone)]
pub struct Batter {
    name: String,
    runs: u32,
    balls_faced: u32,
    on_strike: bool,
}

impl Batter {
    pub fn new(name: &str, runs: u32, balls_faced: u32) -> Self {
        Self {
            name: name.to_string(),
            runs,
            balls_faced,
            on_strike: false,
        }
    }

    // Root* 57 (54)
    pub fn display(&self) -> String {
        let strike_marker = match self.on_strike {
            true => "*",
            false => "",
        };
        format!(
            "{}{} {} ({})",
            self.name, strike_marker, self.runs, self.balls_faced
        )
    }
}

// TODO separate the figures part of a batter and bowler from the batter and bowler struct types?
#[derive(Clone)]
pub struct Bowler {
    name: String,
    overs: Overs,
    wickets: u32,
    runs_conceded: u32,
}

impl Bowler {
    pub fn new(name: &str, overs: Overs, wickets: u32, runs_conceded: u32) -> Self {
        Self {
            name: name.to_string(),
            overs,
            wickets,
            runs_conceded,
        }
    }

    // Broad 4-37 (12.1)
    pub fn display(&self) -> String {
        format!(
            "{} {}-{} ({})",
            self.name,
            self.wickets,
            self.runs_conceded,
            self.overs.display()
        )
    }
}

#[derive(Clone)]
pub struct Overs {
    full_overs: u32,
    spare_balls: u32,
}

impl Overs {
    pub fn display(&self) -> String {
        if self.spare_balls == 0 {
            return format!("{}", self.full_overs);
        }
        return format!("{}.{}", self.full_overs, self.spare_balls);
    }

    pub fn from_str_with_default(s: &str) -> Self {
        match Self::from_str(s) {
            Ok(overs) => overs,
            Err(_) => Self::default(),
        }
    }
}

impl FromStr for Overs {
    type Err = errors::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once(",") {
            None => {
                let o = (s
                    .parse::<u32>()
                    .map_err(|_| errors::Error::ParseError("overs".to_string())))?;
                Ok(Self {
                    full_overs: o,
                    spare_balls: 0,
                })
            }
            Some((o, b)) => {
                let overs = (o
                    .parse::<u32>()
                    .map_err(|_| errors::Error::ParseError("overs".to_string())))?;
                let balls = (b
                    .parse::<u32>()
                    .map_err(|_| errors::Error::ParseError("overs".to_string())))?;
                Ok(Self {
                    full_overs: overs,
                    spare_balls: balls,
                })
            }
        }
    }
}

impl Default for Overs {
    fn default() -> Self {
        Self {
            full_overs: 0,
            spare_balls: 0,
        }
    }
}
