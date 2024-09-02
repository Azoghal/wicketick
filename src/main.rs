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
use wicketick::{
    SimpleSummary, Source, WickeTick, DEFAULT_POLL_INTERVAL, DEFAULT_POLL_INTERVAL_SECS,
};

use std::{
    io::{stdout, Stdout},
    time::Duration,
};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
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
    #[arg(short, long, default_value_t = DEFAULT_POLL_INTERVAL_SECS)]
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

    #[command(about = "use a local copy of a cricinfo summary as the source")]
    LocalCricinfo {
        #[arg(short, long)]
        filename: String,
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

fn phase_from_args<'a>(args: Args) -> Result<(TickerPhase, Option<JoinHandle<()>>), Error> {
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

                    // TODO so we can't stop this boy
                    let (live_stream, stopper) = LiveStream::new(w);

                    Ok((TickerPhase::LiveStream(live_stream), Some(stopper)))
                }
                None => Ok((
                    TickerPhase::MatchSelect(MatchSelect {
                        source: wicketick::Source::Cricinfo { match_id: None },
                    }),
                    None,
                )),
            },
            CliSources::LocalCricinfo { filename } => {
                match std::path::Path::new(&filename).exists() {
                    true => {
                        let source = wicketick::Source::LocalCricinfo { filename };
                        let w = WickeTick {
                            source: source.clone(),
                            summary: None,
                            last_refresh: None,
                            poll_interval: Some(Duration::from_secs(args.time_interval)),
                        };

                        let (live_stream, stopper) = LiveStream::new(w);
                        Ok((TickerPhase::LiveStream(live_stream), Some(stopper)))
                    }
                    false => Err(errors::Error::Todo("file does not exist".to_string())),
                }
            } // _ => Err(errors::Error::Todo("not sure".to_string())),
        },
        None => Ok((TickerPhase::SourceSelect(SourceSelect::new()), None)),
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();

    terminal_preamble()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    let (phase, stopper) = phase_from_args(args)?;

    let mut state: TickerState = TickerState {
        terminal,
        phase,
        stopper,
    };

    // // Move this to correct place
    // let (tx, mut rx) = mpsc::channel(1);

    // // TODO move this to be begun in the correct place
    // // In the end we'll kick off a request
    // tokio::spawn(async move {
    //     let mut summary = SimpleSummary::new();
    //     // just always provide a new simple summary that's got an extra run?
    //     loop {
    //         tx.send(summary.clone()).await.unwrap();
    //         tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    //         summary.current_innings.runs += 1;
    //     }
    // });

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

async fn update(state: &mut TickerState) -> Result<(), Error> {
    // calculate what we want to display

    match &mut state.phase {
        TickerPhase::LiveStream(live_stream) => live_stream.consume_update(),
        _ => {}
    }

    let Some(phase) = state.phase.as_inner_trait() else {
        return Err(Error::Todo("update failed to get trait".to_string()));
    };

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
        Some(phase) => {
            if let Some(join_handle) = &state.stopper {
                join_handle.abort();
            }
            state.phase = phase;
            state.stopper = new_phase.stopper;
        }
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
    stopper: Option<JoinHandle<()>>, // TODO i don't like this
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
// async fn handle_poll(wicketick: &mut WickeTick, wicketick_copy: &Arc<Mutex<WickeTick>>) {
//     if let Some(_) = wicketick.poll_interval {
//         let locked = wicketick_copy.lock().await;
//         if let Some(summary) = locked.summary.clone() {
//             wicketick.summary = Some(summary);
//         }
//     }
// }

// TODO i think this should include a (start) and an (end)
// to control things that should happen as we enter and leave the phase...
trait TickerPhaseTemp {
    fn update(&mut self) -> Result<(), Error>;
    fn draw(
        &mut self,
        terminal: &mut ratatui::terminal::Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<(), Error>;
    fn handle_input(&mut self) -> Result<HandleInputResponse, Error>;
}

// trait Poller<I, O> {
//     // start_poll kicks off some processing that consume will be able to see the value of
//     fn start_poll(poller: I);

//     async fn consume_update(updated_val: O);
// }

struct HandleInputResponse {
    should_close: bool,
    phase: Option<TickerPhase>,
    stopper: Option<JoinHandle<()>>, // TODO really this should be part of the phase... but i think that leads to problems
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
        let widget = Paragraph::new("1. CricInfo\n").white().on_green();
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
                        stopper: None,
                    })
                }
                _ => {}
            }
        }
        Ok(HandleInputResponse {
            should_close,
            phase: None,
            stopper: None,
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
            Source::Cricinfo { match_id: _ } => {
                Paragraph::new("1. pakistan-vs-bangladesh-2nd-test-1442214")
                    .white()
                    .on_green()
            }
            Source::LocalCricinfo { filename } => Paragraph::new(format!("1. {}", filename))
                .white()
                .on_green(),
            Source::_SomeApi {
                base_url: _,
                api_token: _,
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
                    let new_source = Source::Cricinfo {
                        // match_id: Some("1385695".to_string()),
                        match_id: Some("1442214".to_string()),
                        // pakistan-vs-bangladesh-2nd-test-1442214
                    };
                    let wicketick = WickeTick::new(new_source, None);
                    let (live_stream, stopper) = LiveStream::new(wicketick);

                    return Ok(HandleInputResponse {
                        should_close,
                        phase: Some(TickerPhase::LiveStream(live_stream)),
                        stopper: Some(stopper),
                    });
                }
                _ => {}
            }
        }
        Ok(HandleInputResponse {
            should_close,
            phase: None,
            stopper: None,
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
    wicketick: WickeTick,
    // wicketick_copy: Arc<Mutex<WickeTick>>,
    configuration: TickerConfiguration,
    // sender: Sender<SimpleSummary>,
    receiver: Receiver<SimpleSummary>,
}

impl TickerPhaseTemp for LiveStream {
    fn update(&mut self) -> Result<(), Error> {
        // here we want to try receiving from our channel with tokio select
        // which lets us recognise that nothing has changed.
        match self.configuration {
            TickerConfiguration::MinimalTicker => {}
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
                    let basic_text = summary.display();
                    let active_player_text = format!(
                        "{}     {}",
                        summary.active_players.display_batters(),
                        summary.active_players.display_bowlers()
                    );
                    let text = format!("{}          {}", basic_text, active_player_text);
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
            stopper: None,
        })
    }
}

// TODO could genericify this too
impl LiveStream {
    fn start_poll(&mut self, sender: Sender<SimpleSummary>) -> JoinHandle<()> {
        eprintln!("starting poll");
        let w = self.wicketick.clone();
        let mut loop_count = 0;
        let h = tokio::spawn(async move {
            loop {
                match w.refetch().await {
                    Ok(mut summary) => {
                        summary.debug_string = format!("(Ticks: {})", loop_count);
                        loop_count += 1;
                        sender.send(summary.clone()).await.unwrap();
                    }
                    Err(e) => {
                        eprintln!("Oh no: {}", e);
                    }
                }
                if let Some(interval) = w.poll_interval {
                    tokio::time::sleep(interval.clone()).await;
                } else {
                    tokio::time::sleep(DEFAULT_POLL_INTERVAL).await;
                }
            }
        });
        h
    }

    fn consume_update(&mut self) {
        if let Ok(summary) = self.receiver.try_recv() {
            self.wicketick.summary = Some(summary);
        };
    }
}

// type PollStarter = fn() -> JoinHandle<()>;

impl LiveStream {
    // new creates and returns a new phase, also starts the poller, and returns the JoinHandle needed to abort the poller
    fn new(wicketick: WickeTick) -> (Self, JoinHandle<()>) {
        let (tx, rx) = mpsc::channel(1);

        let mut ls = LiveStream {
            wicketick,
            configuration: TickerConfiguration::MinimalTicker,
            receiver: rx,
        };

        let jh = ls.start_poll(tx);

        (ls, jh)
    }
}
