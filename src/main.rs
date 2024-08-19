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
use wicketick::{poll_wicketick, Innings, SimpleSummary, Source, WickeTick};

use std::{
    io::{stdout, Stdout},
    sync::Arc,
    time::Duration,
};
use tokio::{
    select,
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex,
    },
};

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

    // Move this to correct place
    let (tx, mut rx) = mpsc::channel(1);

    // TODO move this to be begun in the correct place
    // In the end we'll kick off a request
    tokio::spawn(async move {
        let mut summary = SimpleSummary::new();
        // just always provide a new simple summary that's got an extra run?
        loop {
            tx.send(summary.clone()).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            summary.current_innings.runs += 1;
        }
    });

    // initialise, block on any necessary setup
    draw(&mut state).await?;

    // main loop
    // TODO I think these main loop things (draw, handle input) should be handled by the phase of the state
    // state.phase.draw()
    // state.phase.handle_input()
    loop {
        // Update
        update(&mut state, &mut rx).await?;

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

// TODO take update receiver?
async fn update(
    state: &mut TickerState,
    update_receiver: &mut Receiver<SimpleSummary>,
) -> Result<(), Error> {
    // calculate what we want to display

    let Some(phase) = state.phase.as_inner_trait() else {
        return Err(Error::Todo("update failed to get trait".to_string()));
    };

    select! {
        Some(value) = update_receiver.recv() => {
            phase.update_summary(value)
        }
    }

    phase.update()
}

async fn draw(state: &mut TickerState) -> Result<(), Error> {
    // calculate what we want to display
    // TODO think if there's a nicer way
    let Some(phase) = state.phase.as_inner_trait() else {
        return Err(Error::Todo("draw failed to get trait".to_string()));
    };
    phase.draw(&mut state.terminal)
}

async fn handle_input(state: &mut TickerState) -> Result<bool, Error> {
    let Some(phase) = state.phase.as_inner_trait() else {
        return Err(Error::Todo("handle input failed to get trait".to_string()));
    };

    let new_phase = phase.handle_input()?;

    match new_phase.phase {
        Some(phase) => state.phase = phase,
        None => {}
    }

    return Ok(new_phase.should_close);
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
// If we need to know things for each of these states, we can add it
struct TickerState {
    terminal: ratatui::terminal::Terminal<CrosstermBackend<Stdout>>,
    phase: TickerPhase,
    // have a tokio channel and use selectors?
    // have a tokio runtime into which we can spawn jobs?
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

// handle poll might also be needed in match select?
async fn handle_poll(wicketick: &mut WickeTick, wicketick_copy: &Arc<Mutex<WickeTick>>) {
    if let Some(_) = wicketick.poll_interval {
        let locked = wicketick_copy.lock().await;
        if let Some(summary) = locked.summary.clone() {
            wicketick.summary = Some(summary);
        }
    }
}

trait TickerPhaseTemp {
    fn update(&mut self) -> Result<(), Error>;
    fn update_summary(&mut self, summary: SimpleSummary) {}
    fn draw(
        &mut self,
        terminal: &mut ratatui::terminal::Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<(), Error>;
    fn handle_input(&mut self) -> Result<HandleInputResponse, Error>;
}

struct HandleInputResponse {
    should_close: bool,
    phase: Option<TickerPhase>,
}

struct SourceSelect {}

impl SourceSelect {
    fn new() -> Self {
        SourceSelect {}
    }
}

impl TickerPhaseTemp for SourceSelect {
    fn update(&mut self) -> Result<(), Error> {
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

    fn handle_input(&mut self) -> Result<HandleInputResponse, Error> {
        let mut should_close = false;
        if let Some(key) = input_key_press()? {
            match key {
                KeyCode::Char('q') => {
                    should_close = true;
                }
                KeyCode::Char('1') => {
                    return Ok(HandleInputResponse {
                        should_close,
                        phase: Some(TickerPhase::MatchSelect(MatchSelect::new(
                            Source::Cricinfo { match_id: None },
                        ))),
                    })
                }
                _ => {}
            }
        }
        Ok(HandleInputResponse {
            should_close,
            phase: None,
        })
    }
}

struct MatchSelect {
    source: Source,
}

impl TickerPhaseTemp for MatchSelect {
    fn update(&mut self) -> Result<(), Error> {
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

    fn handle_input(&mut self) -> Result<HandleInputResponse, Error> {
        let mut should_close = false;
        if let Some(key) = input_key_press()? {
            match key {
                KeyCode::Char('q') => should_close = true,
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
                    return Ok(HandleInputResponse {
                        should_close,
                        phase: Some(TickerPhase::LiveStream(LiveStream::new(
                            new_source.clone(),
                            wicketick,
                        ))),
                    });
                }
                _ => {}
            }
        }
        Ok(HandleInputResponse {
            should_close,
            phase: None,
        })
    }
}

impl MatchSelect {
    fn new(source: Source) -> Self {
        Self { source }
    }
}

// Used to mux the way we lay the summary out in the terminal
#[derive(Copy, Clone)]
enum TickerConfiguration {
    MinimalTicker,
    _RelaxedTicker(Rect),
}

// TODO rename
struct LiveStream {
    source: Source,
    wicketick: WickeTick,
    wicketick_copy: Arc<Mutex<WickeTick>>,
    configuration: TickerConfiguration,
    sender: Sender<SimpleSummary>,
    receiver: Receiver<SimpleSummary>,
}

impl TickerPhaseTemp for LiveStream {
    fn update(&mut self) -> Result<(), Error> {
        // here we want to try receiving from our channel with tokio select
        // which lets us recognise that nothing has changed.
        match self.configuration {
            TickerConfiguration::MinimalTicker => {
                handle_poll(&mut self.wicketick, &self.wicketick_copy);
            }
            TickerConfiguration::_RelaxedTicker(_size) => {}
        }
        Ok(())
    }

    fn update_summary(&mut self, summary: SimpleSummary) {
        self.wicketick.summary = Some(summary)
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

    fn handle_input(&mut self) -> Result<HandleInputResponse, Error> {
        let mut should_close: bool = false;
        if let Some(key) = input_key_press()? {
            match key {
                KeyCode::Char('q') => should_close = true,
                KeyCode::Char('r') => {
                    // TODO kick off a task
                }
                // KeyCode::Char('r') => self.wicketick.refresh()?, // TODO here is where we want to spawn a new task
                _ => {}
            }
        }
        Ok(HandleInputResponse {
            should_close,
            phase: None,
        })
    }
}

impl LiveStream {
    fn new(source: Source, wicketick: WickeTick) -> Self {
        let wicketick_clone = wicketick.clone();
        let (tx, rx) = mpsc::channel(1);
        LiveStream {
            source,
            wicketick,
            wicketick_copy: Arc::new(Mutex::new(wicketick_clone)),
            configuration: TickerConfiguration::MinimalTicker,
            sender: tx,
            receiver: rx,
        }
    }
}
