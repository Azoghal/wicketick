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
use wicketick::{poll_wicketick, Source, WickeTick};

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

fn terminal_preamble() -> Result<(), Error> {
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
fn phase_from_args<'a>(args: Args) -> Result<TickerPhase, Error> {
    // TODO it's a shame that this blocks the main loop from starting till it's fetched
    match args.source {
        Some(source) => match source {
            CliSources::Cricinfo { match_id } => match match_id {
                Some(_) => {
                    let source = wicketick::Source::Cricinfo { match_id: match_id };
                    let w = WickeTick {
                        source: source.clone(),
                        summary: None,
                        last_refresh: None,
                        poll_interval: Some(Duration::from_secs(args.time_interval)),
                    };

                    Ok(TickerPhase::LiveStream(LiveStream::new(source, w)))
                }
                None => Ok(TickerPhase::MatchSelect(MatchSelect {
                    source: wicketick::Source::Cricinfo { match_id: None },
                    should_close: false,
                })),
            },
            _ => Err(errors::Error::Todo("not sure".to_string())),
        },
        None => Ok(TickerPhase::SourceSelect(SourceSelect::new())),
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();

    terminal_preamble()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    let phase = phase_from_args(args)?;

    let mut state: TickerState = TickerState { terminal, phase };

    // initialise, block on any necessary setup
    draw(&mut state).await?;

    // main loop
    // TODO I think these main loop things (draw, handle input) should be handled by the phase of the state
    // state.phase.draw()
    // state.phase.handle_input()
    loop {
        // Update
        update(&mut state).await?;

        // Draw
        draw(&mut state).await?;

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
struct TickerState {
    terminal: ratatui::terminal::Terminal<CrosstermBackend<Stdout>>,
    phase: TickerPhase,
}

// Used to control what functionality the UI needs to be providing
enum TickerPhase {
    SourceSelect(SourceSelect),
    MatchSelect(MatchSelect),
    // TODO probably reduce to just one?
    LiveStream(LiveStream),
}

impl TickerPhase {
    fn as_inner_trait(&mut self) -> Option<&mut dyn TickerPhaseTemp> {
        match self {
            TickerPhase::SourceSelect(inner) => Some(inner),
            TickerPhase::MatchSelect(inner) => Some(inner),
            TickerPhase::LiveStream(inner) => Some(inner),
        }
    }
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
        TickerPhase::SourceSelect(source_select) => {
            // make sure this aligns with what is drawn
            if let Some(key) = input_key_press()? {
                match key {
                    KeyCode::Char('1') => {
                        state.phase = TickerPhase::MatchSelect(MatchSelect::new(Source::Cricinfo {
                            match_id: None,
                        }))
                    }
                    _ => {}
                }
            }
        }
        TickerPhase::MatchSelect(source) => {
            // make sure this aligns with what is drawn
            // Oh no - here we end up having muxed over the phase, but we also need to mux over the source
            if let Some(key) = input_key_press()? {
                match key {
                    KeyCode::Char('1') => {
                        // TODO un hardcode this
                        let new_source = Source::Cricinfo {
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
                            TickerPhase::LiveStream(LiveStream::new(new_source, wicketick));
                    }
                    _ => {}
                }
            }
        }
        TickerPhase::LiveStream(live_stream) => {
            if let Some(key) = input_key_press()? {
                match key {
                    KeyCode::Char('q') => should_break = true,
                    KeyCode::Char('r') => live_stream.wicketick.refresh()?,
                    _ => {}
                }
            }
        }
    }
    return Ok(should_break);
}

fn input_key_press() -> Result<Option<KeyCode>, Error> {
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
        TickerPhase::SourceSelect(source_select) => {}
        TickerPhase::MatchSelect(match_select) => match &match_select.source {
            Source::Cricinfo { match_id } => {}
            Source::_SomeApi {
                base_url,
                api_token,
            } => {}
        },
        TickerPhase::LiveStream(live_stream) => match live_stream.configuration {
            TickerConfiguration::MinimalTicker => {
                handle_poll(&mut live_stream.wicketick, &mut live_stream.wicketick_copy).await;
            }
            TickerConfiguration::_RelaxedTicker(_size) => {}
        },
    };
    Ok(())
}

async fn draw(state: &mut TickerState) -> Result<(), Error> {
    // calculate what we want to display
    // TODO think if there's a nicer way
    let Some(phase) = state.phase.as_inner_trait() else {
        return Err(Error::Todo("boom".to_string()));
    };
    phase.draw(&mut state.terminal)
}

trait TickerPhaseTemp {
    fn update(self) -> Result<(), Error>;
    fn draw(
        &mut self,
        terminal: &mut ratatui::terminal::Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<(), Error>;
    fn handle_input(self) -> Result<Option<TickerPhase>, Error>;
}

struct SourceSelect {
    should_close: bool,
}

impl SourceSelect {
    fn new() -> Self {
        SourceSelect {
            should_close: false,
        }
    }
}

impl TickerPhaseTemp for SourceSelect {
    fn update(self) -> Result<(), Error> {
        Ok(())
    }

    fn draw(
        &mut self,
        terminal: &mut ratatui::terminal::Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<(), Error> {
        let widget = Paragraph::new("1. CricInfo").white().on_green();
        terminal.draw(|frame| {
            let area = frame.size();
            frame.render_widget(widget, area);
        })?;
        Ok(())
    }

    fn handle_input(mut self) -> Result<Option<TickerPhase>, Error> {
        if let Some(key) = input_key_press()? {
            match key {
                KeyCode::Char('q') => {
                    self.should_close = true;
                }
                KeyCode::Char('1') => {
                    return Ok(Some(TickerPhase::MatchSelect(MatchSelect::new(
                        Source::Cricinfo { match_id: None },
                    ))))
                }
                _ => {}
            }
        }
        Ok(None)
    }
}

struct MatchSelect {
    should_close: bool,
    source: Source,
}

impl TickerPhaseTemp for MatchSelect {
    fn update(self) -> Result<(), Error> {
        Ok(())
    }

    fn draw(
        &mut self,
        terminal: &mut ratatui::terminal::Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<(), Error> {
        let widget = match &self.source {
            Source::Cricinfo { match_id } => {
                Paragraph::new("1. Default Match from the hundred 2024")
                    .white()
                    .on_green()
            }
            Source::_SomeApi {
                base_url,
                api_token,
            } => Paragraph::new(format!("Match select not implemented for {}", self.source))
                .white()
                .on_green(),
        };
        terminal.draw(|frame| {
            let area = frame.size();
            frame.render_widget(widget, area);
        })?;
        Ok(())
    }

    fn handle_input(mut self) -> Result<Option<TickerPhase>, Error> {
        if let Some(key) = input_key_press()? {
            match key {
                KeyCode::Char('q') => self.should_close = true,
                KeyCode::Char('1') => {
                    // TODO un hardcode this
                    let new_source = &mut Source::Cricinfo {
                        match_id: Some("1417823".to_string()),
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
                    return Ok(Some(TickerPhase::LiveStream(LiveStream::new(
                        new_source.clone(),
                        wicketick,
                    ))));
                }
                _ => {}
            }
        }
        Ok(None)
    }
}

impl MatchSelect {
    fn new(source: Source) -> Self {
        Self {
            should_close: false,
            source,
        }
    }
}

// TODO rename
struct LiveStream {
    should_close: bool,
    source: Source,
    wicketick: WickeTick,
    wicketick_copy: Arc<Mutex<WickeTick>>,
    configuration: TickerConfiguration,
}

impl TickerPhaseTemp for LiveStream {
    fn update(mut self) -> Result<(), Error> {
        // here we want to yield on our refresh task, and see if we've got a new update
        match self.configuration {
            TickerConfiguration::MinimalTicker => {
                handle_poll(&mut self.wicketick, &self.wicketick_copy);
            }
            TickerConfiguration::_RelaxedTicker(_size) => {}
        }
        Ok(())
    }

    fn draw(
        &mut self,
        terminal: &mut ratatui::terminal::Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<(), Error> {
        let widget = match self.configuration {
            TickerConfiguration::MinimalTicker => {
                if let Some(summary) = &self.wicketick.summary {
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
        };
        terminal.draw(|frame| {
            let area = frame.size();
            frame.render_widget(widget, area);
        })?;
        Ok(())
    }

    fn handle_input(mut self) -> Result<Option<TickerPhase>, Error> {
        if let Some(key) = input_key_press()? {
            match key {
                KeyCode::Char('q') => self.should_close = true,
                KeyCode::Char('r') => self.wicketick.refresh()?, // TODO here is where we want to spawn a new task
                _ => {}
            }
        }
        Ok(None)
    }
}

impl LiveStream {
    fn new(source: Source, wicketick: WickeTick) -> Self {
        let wicketick_clone = wicketick.clone();
        LiveStream {
            should_close: false,
            source,
            wicketick,
            wicketick_copy: Arc::new(Mutex::new(wicketick_clone)),
            configuration: TickerConfiguration::MinimalTicker,
        }
    }
}
