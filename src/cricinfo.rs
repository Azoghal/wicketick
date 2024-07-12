use crate::errors::Error;
use crate::wicketick;
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
}

#[derive(Deserialize, Debug)]
struct LiveState {
    pub innings: Innings,
}

#[derive(Deserialize, Debug)]
struct Innings {
    runs: i32,
    wickets: i32,
    target: Option<i32>,
    overs: String,
}

// TODO impl into wicketick::summary

impl Summary {
    pub fn into(self) -> wicketick::SimpleSummary {
        return wicketick::SimpleSummary {
            current_innings: wicketick::Innings {
                runs: self.live.innings.runs,
                wickets: self.live.innings.wickets,
                overs: self.live.innings.overs,
            },
        };
    }
}
