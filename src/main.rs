use clap::Parser;
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
use wicketick::WickeTick;

use std::{
    io::{stdout, Stdout},
    time,
};
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

async fn wicketick_setup(args: Args) -> Result<WickeTick, Error> {
    // TODO it's a shame that this blocks the main loop from starting till it's fetched
    Ok(WickeTick {
        source: wicketick::Source::Cricinfo {
            match_id: args.match_id,
        },
        summary: None,
        last_refresh: None,
    })
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();
    let state = TickerState::MinimalTicker;

    terminal_setup()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    // TODo made a hash of this - handle input needs to be able to call relevant methods on this,
    let mut wicketick = wicketick_setup(args).await?;

    // main loop
    loop {
        // Draw
        draw(&wicketick, &mut terminal, state)?;

        // Handle input
        let should_break = handle_input(&mut wicketick, state).await?;
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

async fn handle_input(wicketick: &mut WickeTick, _state: TickerState) -> Result<bool, Error> {
    // TODO pattern match on state if we need different interactions
    let mut should_break = false;
    if event::poll(std::time::Duration::from_millis(16))? {
        if let event::Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => should_break = true,
                    KeyCode::Char('r') => wicketick.refresh().await?,
                    _ => {}
                }
            }
        }
    }
    return Ok(should_break);
}

fn draw(
    wicketick: &WickeTick,
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
            if let Some(summary) = &wicketick.summary {
                let text = summary.display();
                Paragraph::new(text.clone()).white().on_black()
            } else {
                Paragraph::new("Loading...").white().on_black()
            }
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
