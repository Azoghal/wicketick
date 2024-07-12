use clap::Parser;
use cricinfo::get_match_summary;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, KeyCode, KeyEventKind},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    },
    layout::Rect,
    style::Stylize,
    widgets::Paragraph,
    Terminal,
};
use wicketick::SimpleSummary;

use std::io::{stdout, Stdout};
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

fn terminal_setup() -> Result<(), Error> {
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    Ok(())
}

fn terminal_teardown() -> Result<(), Error> {
    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

async fn wicketick_setup(args: Args) -> Result<SimpleSummary, Error> {
    // TODO it's a shame that this blocks the main loop from starting till it's fetched
    let Ok(match_summary) = get_match_summary(args.match_id).await else {
        return Err(Error::Todo("Failed to get match summary".to_string()));
    };
    Ok(match_summary)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();
    let state = TickerState::MinimalTicker;

    terminal_setup()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    let match_summary = wicketick_setup(args).await?;

    // main loop
    loop {
        // Draw
        draw(&match_summary, &mut terminal, state)?;

        // Handle input
        let should_break = handle_input(state)?;
        if should_break {
            break;
        }
    }

    terminal_teardown()?;
    Ok(())
}

// If we need to know things for each of these states, we can add it
#[derive(Copy, Clone)]
enum TickerState {
    _MatchSelect,
    MinimalTicker,
    _RelaxedTicker(Rect),
}

fn handle_input(_state: TickerState) -> Result<bool, Error> {
    // TODO pattern match on state if we need different interactions
    let mut should_break = false;
    if event::poll(std::time::Duration::from_millis(16))? {
        if let event::Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => should_break = true,
                    _ => {}
                }
            }
        }
    }
    return Ok(should_break);
}

fn draw(
    ref match_summary: &SimpleSummary,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: TickerState,
) -> Result<(), Error> {
    // calculate what we want to display
    // TODO instead of returning the text that we whack in the widget, this pattern match can produce us the widget we render
    let widget = match state {
        TickerState::_MatchSelect => Paragraph::new("Match select not implemented")
            .white()
            .on_green(),
        TickerState::MinimalTicker => {
            let text = match_summary.display();
            Paragraph::new(text.clone()).white().on_black()
        }
        TickerState::_RelaxedTicker(_size) => Paragraph::new("Relaxed ticker not implemented")
            .white()
            .on_blue(),
    };
    terminal.draw(|frame| {
        let area = frame.size();
        frame.render_widget(widget, area);
    })?;
    Ok(())
}
