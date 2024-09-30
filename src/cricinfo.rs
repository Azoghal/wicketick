use std::fmt;
use std::marker::PhantomData;

use crate::errors::Error;
use crate::wicketick::{self};
use reqwest;
use serde::{Deserialize, Deserializer};

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

pub fn load_match_summary(filename: String) -> Result<wicketick::SimpleSummary, Error> {
    let file = std::fs::File::open(filename)?;

    let match_summary: Summary = serde_json::from_reader(file)?;

    let wicketick = match_summary.into();

    return Ok(wicketick);
}

fn parse_u32(bob: String) -> u32 {
    bob.parse::<u32>()
        .map_err(|e| {
            eprintln!("failed to parse u32 {}", e);
            Some(0)
        })
        .unwrap()
}

// Layout in structs all the info from the Json they host, that we actually care about
// Then we can automatically deserialise it, and we're good to go

#[derive(Deserialize, Debug)]
struct Summary {
    live: LiveState,
    // centre: Centre,
    team: Vec<Team>,
}

impl Summary {
    // return the known_as from the teams listing
    fn lookup_player_name(&self, player_id: &str) -> String {
        for team in &self.team {
            for player in &team.player {
                if player_id == player.player_id {
                    return player.known_as.clone();
                }
            }
        }
        return "Unkown".to_string();
    }
}

#[derive(Deserialize, Debug)]
struct LocalSummary {
    live: LiveState,
}

// #[derive(Deserialize, Debug)]
// struct Centre {
//     pub batting: Vec<Batter>,
//     pub bowling: Vec<Bowler>,
// }

#[derive(Deserialize, Debug)]
struct Player {
    known_as: String,
    popular_name: String,
    player_id: String,
}

#[derive(Deserialize, Debug, Clone)]
struct Batter {
    // #[serde(deserialize_with = "deserialize_stringy_int")]
    balls_faced: String,
    live_current_name: String,
    #[serde(deserialize_with = "deserialize_stringy_int")]
    runs: u32,
    player_id: String,
    team_id: u32,
}

impl Batter {
    fn to_wicketick(self, name: &str) -> wicketick::Batter {
        let balls_faced = parse_u32(self.balls_faced);
        wicketick::Batter::new(
            name,
            self.runs,
            balls_faced,
            self.live_current_name == "striker",
        )
    }
}

#[derive(Deserialize, Debug, Clone)]
struct Bowler {
    overs: String,
    live_current_name: String,
    #[serde(deserialize_with = "deserialize_stringy_int")]
    conceded: u32,
    #[serde(deserialize_with = "deserialize_stringy_int")]
    wickets: u32,
    player_id: String,
    team_id: u32,
}

impl Bowler {
    fn to_wicketick(self, name: &str) -> wicketick::Bowler {
        wicketick::Bowler::new(
            name,
            wicketick::Overs::from_str_with_default(&self.overs),
            self.wickets,
            self.conceded,
        )
    }
}

#[derive(Deserialize, Debug)]
struct LiveState {
    pub innings: Innings,
    pub batting: Vec<Batter>,
    pub bowling: Vec<Bowler>,
    // pub fow: Vec<FoW>,
    // pub status: String,
}

#[derive(Deserialize, Debug)]
struct Innings {
    runs: u32,
    wickets: u32,
    target: u32,
    overs: String,
}

// #[derive(Deserialize, Debug)]
// struct FoW {
//     fow_order: u8,
// }

#[derive(Deserialize, Debug)]
struct Team {
    player: Vec<Player>,
    team_id: String,
    team_name: String,
    team_short_name: String,
}

impl Summary {
    pub fn into(self) -> wicketick::SimpleSummary {
        let bowler_count = self.live.bowling.len();
        let batter_count = self.live.batting.len();

        let map_batter = |b: Batter| {
            let id = b.clone().player_id;
            Some(b.to_wicketick(&self.lookup_player_name(&id)))
        };
        let map_bowler = |b: Bowler| {
            let id = b.clone().player_id;
            Some(b.to_wicketick(&self.lookup_player_name(&id)))
        };

        let active_players = match bowler_count + batter_count {
            4 => wicketick::ActivePlayers {
                batter_one: map_batter(self.live.batting[0].clone()),
                batter_two: map_batter(self.live.batting[1].clone()),
                bowler_one: map_bowler(self.live.bowling[0].clone()),
                bowler_two: map_bowler(self.live.bowling[1].clone()),
            },
            _ => wicketick::ActivePlayers::default(),
        };

        return wicketick::SimpleSummary {
            current_innings: wicketick::Innings {
                runs: self.live.innings.runs as u32,
                wickets: self.live.innings.wickets as u32,
                overs: self.live.innings.overs,
                target: match self.live.innings.target {
                    0 => None,
                    n => Some(n),
                },
            },
            active_players,
            debug_string: "".to_string(),
        };
    }
}

// reference https://serde.rs/stream-array.html
// could make this genric where T (instead of u32) as long as we have a trait for parsing from string?
fn deserialize_stringy_int<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringyIntVisitor(PhantomData<fn() -> u32>);

    impl<'de> serde::de::Visitor<'de> for StringyIntVisitor {
        /// Return type of this visitor. This visitor computes the max of a
        /// sequence of values of type T, so the type of the maximum is T.
        type Value = u32;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an integer or a string of an integer")
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let x_safe: u32 = value
                .try_into()
                .map_err(|e| serde::de::Error::custom(format!("{:?}", e)))?;
            Ok(x_safe)
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let x_safe: u32 = value
                .try_into()
                .map_err(|e| serde::de::Error::custom(format!("{:?}", e)))?;
            Ok(x_safe)
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let val = value
                .parse::<u32>()
                .map_err(|e| serde::de::Error::custom(format!("invalid integer: {:?}", e)))?;
            Ok(val)
        }
    }

    // Create the visitor and ask the deserializer to drive it. The
    // deserializer will call visitor.visit_seq() if a seq is present in
    // the input data.
    let visitor = StringyIntVisitor(PhantomData);
    deserializer.deserialize_any(visitor)
}
