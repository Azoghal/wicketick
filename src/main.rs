use clap::{Parser, Subcommand};
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
use wicketick::{poll_wicketick, Source, WickeTick, DEFAULT_POLL_INTERVAL};

use std::{
    io::{stdout, Stdout},
    sync::Arc,
    time::Duration,
};
use tokio::sync::Mutex;

pub mod errors;
use errors::Error;

pub mod cricinfo;
pub mod wicketick;

// todo this needs to be updated to account for different sources
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    // Id of match to display
    #[command(subcommand)]
    source: Option<CliSources>,

    // polling interval in seconds
    #[arg(short, long, default_value_t = 30)]
    time_interval: u64,
    // Obviously there could be all sorts of things we do here
}

#[derive(Subcommand, Debug)]
enum CliSources {
    #[command(about = "use cricinfo as the source")]
    Cricinfo {
        #[arg(short, long, default_value=None)]
        match_id: Option<String>,
    },
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

// TODO this needs to be called at the right time, rather than when parsing args
fn phase_from_args(args: Args) -> Result<TickerPhase, Error> {
    // TODO it's a shame that this blocks the main loop from starting till it's fetched
    match args.source {
        Some(source) => match source {
            CliSources::Cricinfo { match_id } => match match_id {
                Some(_) => {
                    let w = WickeTick {
                        source: wicketick::Source::Cricinfo { match_id: match_id },
                        summary: None,
                        last_refresh: None,
                        poll_interval: Some(Duration::from_secs(args.time_interval)),
                    };
                    let w_c = Arc::new(Mutex::new(w.clone()));
                    let config = TickerConfiguration::MinimalTicker;
                    Ok(TickerPhase::Display(w, w_c, config))
                }
                None => Ok(TickerPhase::MatchSelect(wicketick::Source::Cricinfo {
                    match_id: None,
                })),
            },
            _ => Err(errors::Error::Todo("not sure".to_string())),
        },
        None => Ok(TickerPhase::SourceSelect),
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();
    let mut state = TickerState {
        phase: phase_from_args(args)?,
    };

    // We can skip certain phases depending on the args given
    // We see if a source is provided
    // If so, we ask that source to parse the rest of it's arguments
    // and potentially move into the match select phase

    terminal_setup()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    // initialise, block on any necessary setup
    draw(&mut terminal, &state).await?;

    // main loop
    // TODO I think these main loop things (draw, handle input) should be handled by the phase of the state
    // state.phase.draw()
    // state.phase.handle_input()
    loop {
        // Update
        update(&mut state).await?;

        // Draw
        draw(&mut terminal, &state).await?;

        // Handle input
        let should_break = handle_input(&mut state).await?;
        if should_break {
            break;
        }
    }

    terminal_teardown()?;
    Ok(())
}

// If we need to know things for each of these states, we can add it
#[derive(Clone)]
struct TickerState {
    phase: TickerPhase,
}

// Used to control what functionality the UI needs to be providing
#[derive(Clone)]
enum TickerPhase {
    SourceSelect,
    MatchSelect(Source),
    // TODO probably reduce to just one?
    Display(WickeTick, Arc<Mutex<WickeTick>>, TickerConfiguration),
}

// Used to mux the way we lay the summary out in the terminal
#[derive(Copy, Clone)]

enum TickerConfiguration {
    MinimalTicker,
    _RelaxedTicker(Rect),
}
// handle poll might also be needed in match select?
async fn handle_poll(wicketick: &mut WickeTick, wicketick_copy: &Arc<Mutex<WickeTick>>) {
    if let Some(_) = wicketick.poll_interval {
        let locked = wicketick_copy.lock().await;
        if let Some(summary) = locked.summary.clone() {
            wicketick.summary = Some(summary);
        }
    }
}

async fn handle_input(state: &mut TickerState) -> Result<bool, Error> {
    let mut should_break = false;
    match &mut state.phase {
        TickerPhase::SourceSelect => {
            // make sure this aligns with what is drawn
            if let Some(key) = input_key_press().await? {
                match key {
                    KeyCode::Char('1') => {
                        state.phase = TickerPhase::MatchSelect(Source::Cricinfo { match_id: None })
                    }
                    _ => {}
                }
            }
        }
        TickerPhase::MatchSelect(source) => {
            // make sure this aligns with what is drawn
            // Oh no - here we end up having muxed over the phase, but we also need to mux over the source
            if let Some(key) = input_key_press().await? {
                match key {
                    KeyCode::Char('1') => {
                        // TODO un hardcode this
                        let new_source = &mut Source::Cricinfo {
                            match_id: Some("1443995".to_string()),
                        };
                        // TODO separate this out
                        let wicketick = WickeTick::new(new_source.clone(), None);
                        let interval = wicketick.clone().poll_interval;
                        let configuration = TickerConfiguration::MinimalTicker;
                        let wicketick_copy = Arc::new(Mutex::new(wicketick.clone()));
                        if let Some(int) = interval {
                            let data_clone = Arc::clone(&wicketick_copy);
                            // TODO some way to kill thread when needed? let (tx, mut rx) = mpsc::channel(1);
                            tokio::spawn(poll_wicketick(data_clone, int));
                        }
                        state.phase =
                            TickerPhase::Display(wicketick, wicketick_copy, configuration);
                    }
                    _ => {}
                }
            }
        }
        TickerPhase::Display(wicketick, wicketick_copy, configuration) => {
            if let Some(key) = input_key_press().await? {
                match key {
                    KeyCode::Char('q') => should_break = true,
                    KeyCode::Char('r') => wicketick.refresh().await?,
                    _ => {}
                }
            }
        }
    }
    return Ok(should_break);
}

async fn input_key_press() -> Result<Option<KeyCode>, Error> {
    if event::poll(std::time::Duration::from_millis(16))? {
        if let event::Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                return Ok(Some(key.code));
            }
        }
    }
    Ok(None)
}

async fn update(state: &mut TickerState) -> Result<(), Error> {
    // calculate what we want to display
    // TODO think if there's a nicer way
    let widget = match &mut state.phase {
        TickerPhase::SourceSelect => {}
        TickerPhase::MatchSelect(source) => match source {
            Source::Cricinfo { match_id } => {}
            Source::_SomeApi {
                base_url,
                api_token,
            } => {}
        },
        TickerPhase::Display(wicketick, wicketick_copy, configuration) => match configuration {
            TickerConfiguration::MinimalTicker => {
                handle_poll(wicketick, wicketick_copy).await;
            }
            TickerConfiguration::_RelaxedTicker(_size) => {}
        },
    };
    Ok(())
}

async fn draw(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &TickerState,
) -> Result<(), Error> {
    // calculate what we want to display
    // TODO think if there's a nicer way
    let widget = match &state.phase {
        TickerPhase::SourceSelect => Paragraph::new("1. CricInfo").white().on_green(),
        TickerPhase::MatchSelect(source) => match source {
            Source::Cricinfo { match_id } => {
                Paragraph::new("1. Default Match from the hundred 2024")
                    .white()
                    .on_green()
            }
            Source::_SomeApi {
                base_url,
                api_token,
            } => Paragraph::new(format!("Match select not implemented for {}", source))
                .white()
                .on_green(),
        },
        TickerPhase::Display(wicketick, _, configuration) => match configuration {
            TickerConfiguration::MinimalTicker => {
                if let Some(summary) = &wicketick.summary {
                    let text = summary.display();
                    Paragraph::new(text.clone()).white().on_black()
                } else {
                    Paragraph::new("Loading...").white().on_black()
                }
            }
            TickerConfiguration::_RelaxedTicker(_size) => {
                Paragraph::new("Relaxed ticker not implemented")
                    .white()
                    .on_blue()
            }
        },
    };
    terminal.draw(|frame| {
        let area = frame.size();
        frame.render_widget(widget, area);
    })?;
    Ok(())
}
