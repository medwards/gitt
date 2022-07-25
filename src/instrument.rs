use std::{
    fmt::Display,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Timing {
    pub name: String,
    pub index: usize,
    pub duration: Duration,
}

impl Timing {
    pub fn new(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }

    pub fn record_max(&mut self, instant: Instant, index: usize) {
        let duration = instant.elapsed();
        if self.duration < duration {
            self.duration = duration;
            self.index = index;
        }
    }
}

impl Display for Timing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {}ms (index {})",
            self.name,
            self.duration.as_millis(),
            self.index
        )
    }
}
