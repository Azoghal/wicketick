use clap::Parser;
use cricinfo::get_match_summary;
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
use tokio;

pub mod errors;
use errors::Error;

pub mod cricinfo;
pub mod wicketick;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    // Id of match to display
    #[arg(short, long)]
    match_id: String,
    // Obviously there could be all sorts of things we do here
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();

    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    // TODO it's a shame that this blocks the main loop from starting till it's fetched
    let Ok(match_summary) = get_match_summary(args.match_id).await else {
        return Err(Error::Todo("Failed to get match summary".to_string()));
    };

    let text = match_summary.display();

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
