use std::sync::mpsc::{channel, Receiver, RecvError};
use std::time::Duration;

use crossterm::event::{poll, read, Event as CrosstermEvent, KeyEvent};
use tui::widgets::ListState;

use crate::model::AppModel;

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
    // TODO: these should be in the model
    pub list_state: ListState,
    // TODO: this has weird behaviour since its derived from the view but if its not a fixed layout
    // constraint then we don't know the height until render time.
    // Maybe draw once /wo the content before the loop starts to get the initial height?
    pub list_height: usize,
}

impl EventHandler {
    pub fn new(tick_rate: Duration, list_state: ListState, list_height: usize) -> Self {
        Self {
            receiver: event_receiver(tick_rate),
            list_state,
            list_height,
        }
    }

    pub fn update_model(&mut self, model: &mut AppModel) -> Result<(), RecvError> {
        match self.receiver.recv()? {
            Event::Input(event) => match event.code {
                crossterm::event::KeyCode::Char('q') => {
                    model.app_state = crate::model::AppState::Finished;
                }
                crossterm::event::KeyCode::Char('g') => {
                    self.list_state.select(Some(0));
                    model.go_to_first();
                }
                crossterm::event::KeyCode::Char('G') => {
                    self.list_state.select(Some(self.list_height - 1));
                    model.go_to_last();
                    (0..self.list_height)
                        .into_iter()
                        .for_each(|_| model.decrement());
                }
                crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') => {
                    if model.remaining(self.list_height) == 0 {
                        return Ok(());
                    }
                    match self.list_state.selected() {
                        Some(index) => self.list_state.select(Some(index + 1)),
                        None => self.list_state.select(Some(0)),
                    };
                    if self.list_state.selected().unwrap_or(0) >= self.list_height {
                        self.list_state.select(Some(self.list_height - 1));
                        model.increment();
                    }
                }
                crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') => {
                    if self.list_state.selected().unwrap_or(self.list_height) == 0 {
                        model.decrement();
                    }
                    match self.list_state.selected() {
                        Some(index) => self.list_state.select(Some(index.saturating_sub(1))),
                        None => self
                            .list_state
                            .select(Some(self.list_height.saturating_sub(1))),
                    };
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
