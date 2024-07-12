use clap::Parser;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, KeyCode, KeyEventKind},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    },
    style::Stylize,
    widgets::Paragraph,
    Terminal,
};
use reqwest;
use serde::Deserialize;
use std::io::stdout;
use tokio;

pub mod errors;
use errors::Error;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    // Id of match to display
    #[arg(short, long)]
    match_id: String,
    // Obviously there could be all sorts of things we do here
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();

    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    let prelude = "I'm going to be following the match with id:";
    let mut text = format!("{prelude}{}", args.match_id);

    // TODO it's a shame that this blocks the main loop from starting till it's fetched
    let match_summary = get_match_summary(args.match_id).await;
    match match_summary {
        Ok(summary) => {
            let runs = summary.live.innings.runs;
            let wickets = summary.live.innings.wickets;
            // TODO easier if we give the struct the display methods
            text = format!("{} - {}", runs, wickets);
        }
        Err(e) => {}
    }

    // main loop
    loop {
        // Draw
        terminal.draw(|frame| {
            let area = frame.size();
            frame.render_widget(Paragraph::new(text.clone()).white().on_blue(), area);
        })?;

        // Handle input
        if event::poll(std::time::Duration::from_millis(16))? {
            if let event::Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => break,
                        _ => {}
                    }
                }
            }
        }
    }

    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

// example match ids:
// finished test match = 1385691
// currently in progress t20 = 1410472
async fn get_match_summary(match_id: String) -> Result<EspnCricInfoMatchSummary, Error> {
    let body = reqwest::get(format!(
        "https://www.espncricinfo.com/matches/engine/match/{}.json",
        match_id
    ))
    .await?
    .text()
    .await?;

    let match_summary: EspnCricInfoMatchSummary = serde_json::from_str(&body)?;

    return Ok(match_summary);
}

// A proof of concept espn cricinfo struct
#[derive(Deserialize, Debug)]
struct EspnCricInfoMatchSummary {
    live: EspnCricInfoLive,
}

#[derive(Deserialize, Debug)]
struct EspnCricInfoLive {
    innings: EspnCricInfoInnings,
}

#[derive(Deserialize, Debug)]
struct EspnCricInfoInnings {
    runs: i32,
    wickets: i32,
    target: Option<i32>,
}
