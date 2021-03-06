use std::sync::mpsc::{channel, Receiver, RecvError};
use std::time::Duration;

use crossterm::event::{poll, read, Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};

use crate::model::{AppModel, AppState};

pub enum Event<I> {
    Input(I),
    Resize(u16),
    Failure,
    Tick,
}

impl Event<KeyEvent> {
    pub fn listen(timeout: Duration) -> Result<Option<Self>, String> {
        if poll(timeout).map_err(|e| e.to_string())? {
            match read().map_err(|e| e.to_string())? {
                CrosstermEvent::Key(key) => Ok(Some(Event::Input(key))),
                CrosstermEvent::Resize(_columns, rows) => Ok(Some(Event::Resize(rows))),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
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
                if tx.send(Event::Tick).is_ok() {
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
        loop {
            match self.receiver.recv()? {
                Event::Input(event) => {
                    if model.app_state == AppState::Commits {
                        match event {
                            KeyEvent {
                                code: KeyCode::Char('q'),
                                ..
                            } => {
                                model.app_state = AppState::Finished;
                            }
                            KeyEvent {
                                code: KeyCode::Tab, ..
                            } => {
                                // TODO: statemachine for app state progression
                                model.app_state = AppState::Details;
                            }
                            // Commit navigation
                            KeyEvent {
                                code: KeyCode::Char('g'),
                                ..
                            } => {
                                model.go_to_first_revision();
                            }
                            KeyEvent {
                                code: KeyCode::Char('G'),
                                ..
                            } => {
                                model.go_to_last_revision();
                            }
                            KeyEvent {
                                code: KeyCode::Down,
                                ..
                            }
                            | KeyEvent {
                                code: KeyCode::Char('j'),
                                ..
                            } => {
                                model.increment_revision();
                            }
                            KeyEvent {
                                code: KeyCode::Up, ..
                            }
                            | KeyEvent {
                                code: KeyCode::Char('k'),
                                ..
                            } => {
                                model.decrement_revision();
                            }
                            _ => {}
                        }
                    } else if model.app_state == AppState::Details {
                        match event {
                            KeyEvent {
                                code: KeyCode::Char('q'),
                                ..
                            } => {
                                model.app_state = AppState::Finished;
                            }
                            KeyEvent {
                                code: KeyCode::Tab, ..
                            } => {
                                // TODO: statemachine for app state progression
                                model.app_state = AppState::Commits;
                            }
                            // Details navigation
                            KeyEvent {
                                code: KeyCode::Char('g'),
                                ..
                            } => {
                                model.go_to_first_diff_line();
                            }
                            KeyEvent {
                                code: KeyCode::Char('G'),
                                ..
                            } => {
                                model.go_to_last_diff_line();
                            }
                            KeyEvent {
                                code: KeyCode::Down,
                                ..
                            }
                            | KeyEvent {
                                code: KeyCode::Char('j'),
                                ..
                            } => {
                                model.increment_diff_line();
                            }
                            KeyEvent {
                                code: KeyCode::Up, ..
                            }
                            | KeyEvent {
                                code: KeyCode::Char('k'),
                                ..
                            } => {
                                model.decrement_diff_line();
                            }

                            KeyEvent {
                                code: KeyCode::PageDown,
                                ..
                            }
                            | KeyEvent {
                                code: KeyCode::Char('f'),
                                modifiers: KeyModifiers::CONTROL,
                            } => {
                                let (_, window_length, _) = model.diff_line_scroll();
                                for _ in 0..window_length {
                                    model.increment_diff_line();
                                }
                            }

                            KeyEvent {
                                code: KeyCode::PageUp,
                                ..
                            }
                            | KeyEvent {
                                code: KeyCode::Char('b'),
                                modifiers: KeyModifiers::CONTROL,
                            } => {
                                let (_, window_length, _) = model.diff_line_scroll();
                                for _ in 0..window_length {
                                    model.decrement_diff_line();
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Event::Resize(_rows) => {} // Let the drawing code handle this
                Event::Failure => {
                    model.app_state = crate::model::AppState::Finished;
                }
                Event::Tick => continue,
            };
            break;
        }
        Ok(())
    }
}
