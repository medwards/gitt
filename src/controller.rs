use std::sync::mpsc::{channel, Receiver, RecvError};
use std::time::Duration;

use crossterm::event::{poll, read, Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};

use crate::model::{AppModel, AppState};

pub enum Event<I> {
    Input(I),
    Failure,
    Tick,
}

impl Event<KeyEvent> {
    pub fn listen(timeout: Duration) -> Result<Option<Self>, String> {
        if poll(timeout).map_err(|e| e.to_string())? {
            if let CrosstermEvent::Key(key) = read().map_err(|e| e.to_string())? {
                return Ok(Some(Event::Input(key)));
            }
        }
        Ok(None)
    }
}

pub fn event_receiver(tick_rate: Duration) -> Receiver<Event<KeyEvent>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let mut last_tick = std::time::Instant::now();
        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            match Event::listen(timeout) {
                Ok(Some(e)) => tx.send(e).expect("Failed to send event"),
                Err(_) => tx.send(Event::Failure).expect("Failed to send event"),
                _ => {}
            }

            if last_tick.elapsed() >= tick_rate {
                if let Ok(_) = tx.send(Event::Tick) {
                    last_tick = std::time::Instant::now();
                }
            }
        }
    });
    rx
}

pub struct EventHandler {
    receiver: Receiver<Event<KeyEvent>>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        Self {
            receiver: event_receiver(tick_rate),
        }
    }

    pub fn update_model(&mut self, model: &mut AppModel) -> Result<(), RecvError> {
        match self.receiver.recv()? {
            Event::Input(event) => match (model.app_state, event) {
                (
                    _,
                    KeyEvent {
                        code: KeyCode::Char('q'),
                        ..
                    },
                ) => {
                    model.app_state = AppState::Finished;
                }
                (
                    _,
                    KeyEvent {
                        code: KeyCode::Tab, ..
                    },
                ) => {
                    // TODO: statemachine for app state progression
                    if model.app_state == AppState::Commits {
                        model.app_state = AppState::Details;
                    } else if model.app_state == AppState::Details {
                        model.app_state = AppState::Commits;
                    }
                }
                // Commit navigation
                (
                    AppState::Commits,
                    KeyEvent {
                        code: KeyCode::Char('g'),
                        ..
                    },
                ) => {
                    model.go_to_first_revision();
                }
                (
                    AppState::Commits,
                    KeyEvent {
                        code: KeyCode::Char('G'),
                        ..
                    },
                ) => {
                    model.go_to_last_revision();
                }
                (
                    AppState::Commits,
                    KeyEvent {
                        code: KeyCode::Down,
                        ..
                    },
                )
                | (
                    AppState::Commits,
                    KeyEvent {
                        code: KeyCode::Char('j'),
                        ..
                    },
                ) => {
                    model.increment_revision();
                }
                (
                    AppState::Commits,
                    KeyEvent {
                        code: KeyCode::Up, ..
                    },
                )
                | (
                    AppState::Commits,
                    KeyEvent {
                        code: KeyCode::Char('k'),
                        ..
                    },
                ) => {
                    model.decrement_revision();
                }
                // Details navigation
                (
                    AppState::Details,
                    KeyEvent {
                        code: KeyCode::Char('g'),
                        ..
                    },
                ) => {
                    model.go_to_first_diff_line();
                }
                (
                    AppState::Details,
                    KeyEvent {
                        code: KeyCode::Char('G'),
                        ..
                    },
                ) => {
                    model.go_to_last_diff_line();
                }
                (
                    AppState::Details,
                    KeyEvent {
                        code: KeyCode::Down,
                        ..
                    },
                )
                | (
                    AppState::Details,
                    KeyEvent {
                        code: KeyCode::Char('j'),
                        ..
                    },
                ) => {
                    model.increment_diff_line();
                }
                (
                    AppState::Details,
                    KeyEvent {
                        code: KeyCode::Up, ..
                    },
                )
                | (
                    AppState::Details,
                    KeyEvent {
                        code: KeyCode::Char('k'),
                        ..
                    },
                ) => {
                    model.decrement_diff_line();
                }
                (
                    AppState::Details,
                    KeyEvent {
                        code: KeyCode::PageDown,
                        ..
                    }
                    | KeyEvent {
                        code: KeyCode::Char('f'),
                        modifiers: KeyModifiers::CONTROL,
                    },
                ) => {
                    let (_, window_length, _) = model.diff_line_scroll();
                    for _ in 0..window_length {
                        model.increment_diff_line();
                    }
                }
                (
                    AppState::Details,
                    KeyEvent {
                        code: KeyCode::PageUp,
                        ..
                    }
                    | KeyEvent {
                        code: KeyCode::Char('b'),
                        modifiers: KeyModifiers::CONTROL,
                    },
                ) => {
                    let (_, window_length, _) = model.diff_line_scroll();
                    for _ in 0..window_length {
                        model.decrement_diff_line();
                    }
                }
                _ => {}
            },
            Event::Failure => {
                model.app_state = crate::model::AppState::Finished;
            }
            Event::Tick => {}
        };
        Ok(())
    }
}
