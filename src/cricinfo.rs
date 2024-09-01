use std::str::FromStr;

use crate::errors::Error;
use crate::wicketick::{self, ActivePlayers};
use reqwest;
use serde::Deserialize;

// example match ids:
// finished test match = 1385691
// currently in progress t20 = 1410472
pub async fn get_match_summary(match_id: String) -> Result<wicketick::SimpleSummary, Error> {
    let body = reqwest::get(format!(
        "https://www.espncricinfo.com/matches/engine/match/{}.json",
        match_id
    ))
    .await?
    .text()
    .await?;

    let match_summary: Summary = serde_json::from_str(&body)?;

    let wicketick = match_summary.into();

    return Ok(wicketick);
}

// Layout in structs all the info from the Json they host, that we actually care about
// Then we can automatically deserialise it, and we're good to go

#[derive(Deserialize, Debug)]
struct Summary {
    live: LiveState,
    centre: Centre,
}

#[derive(Deserialize, Debug)]
struct Centre {
    pub batting: Vec<CentreBatter>,
    pub bowling: Vec<CentreBowler>,
}

#[derive(Deserialize, Debug, Clone)]
struct CentreBatter {
    balls_faced: u32,
    known_as: String,
    live_current_name: String,
    popular_name: String,
    runs: u32,
}

impl CentreBatter {
    fn into(self) -> wicketick::Batter {
        wicketick::Batter::new(&self.known_as, self.runs, self.balls_faced)
    }
}

#[derive(Deserialize, Debug, Clone)]
struct CentreBowler {
    overs: String,
    known_as: String,
    live_current_name: String,
    popular_name: String,
    conceded: u32,
    wickets: u32,
}

impl CentreBowler {
    fn into(self) -> wicketick::Bowler {
        wicketick::Bowler::new(
            &self.known_as,
            wicketick::Overs::from_str_with_default(&self.overs),
            self.wickets,
            self.conceded,
        )
    }
}

#[derive(Deserialize, Debug)]
struct LiveState {
    pub innings: Innings,
    // pub batting: Batting,
    // pub bowling: Bowling,
    pub fow: Vec<FoW>,
    pub status: String,
}

#[derive(Deserialize, Debug)]
struct Innings {
    runs: i32,
    wickets: i32,
    target: Option<i32>,
    overs: String,
}

#[derive(Deserialize, Debug)]
struct FoW {
    fow_order: u8,
}

struct Team {}

impl Summary {
    pub fn into(self) -> wicketick::SimpleSummary {
        let bowler_count = self.centre.bowling.len();
        let batter_count = self.centre.batting.len();

        let active_players = match bowler_count + batter_count {
            4 => wicketick::ActivePlayers {
                batter_one: Some(self.centre.batting[0].clone().into()),
                batter_two: Some(self.centre.batting[1].clone().into()),
                bowler_one: Some(self.centre.bowling[0].clone().into()),
                bowler_two: Some(self.centre.bowling[1].clone().into()),
            },
            _ => wicketick::ActivePlayers::default(),
        };

        return wicketick::SimpleSummary {
            current_innings: wicketick::Innings {
                runs: self.live.innings.runs as u32,
                wickets: self.live.innings.wickets as u32,
                overs: self.live.innings.overs,
            },
            active_players,
            debug_string: "".to_string(),
        };
    }
}
