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
use std::io::stdout;

use reqwest;

use tokio;

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
                        KeyCode::Char('a') => {
                            let windies_snippet = get_windies_match().await;
                            match windies_snippet {
                                Ok(snippet) => {
                                    text = format!("{prelude}{}\n{}", args.match_id, snippet);
                                }
                                Err(e) => {
                                    text = format!("{prelude}{}\n!{:?}", args.match_id, e);
                                }
                            }
                        }
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

async fn get_windies_match() -> Result<String, reqwest::Error> {
    let body = reqwest::get("https://www.espncricinfo.com/matches/engine/match/1385691.json")
        .await?
        .text()
        .await?;

    let snippet: String = body
        .char_indices()
        .take_while(|(i, _)| *i < 25)
        .map(|(_, c)| c)
        .collect();

    return Ok(snippet);
}
