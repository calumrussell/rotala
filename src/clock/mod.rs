use std::sync::Arc;
use std::sync::Mutex;
use std::vec::IntoIter;

use crate::types::{DateTime, Frequency};

#[doc(hidden)]
#[derive(Debug)]
pub struct ClockInner {
    //We have a position and Vec because we should be able to return an iterator without changing
    //the state of the Clock
    pos: usize,
    dates: Vec<DateTime>,
}

/// Used to synchronize time between components.
/// 
/// Shared clock simplifies the synchronization of components that rely on data sources during
/// backtests. 
/// 
/// [Clock] is thread-safe and wrapped in [Arc] so can be cheaply cloned and references held across
/// the application.
#[derive(Debug)]
pub struct Clock {
    inner: Arc<Mutex<ClockInner>>,
}

impl Clone for Clock {
    fn clone(&self) -> Self {
        Clock {
            inner: Arc::clone(&self.inner),
        }
    }
}

unsafe impl Send for Clock {}

impl Clock {
    pub fn now(&self) -> DateTime {
        let inner = self.inner.lock().unwrap();
        //This cannot trigger an error because the error will be thrown when the client ticks to an
        //invalid position
        *inner.dates.get(inner.pos).unwrap()
    }

    pub fn has_next(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.pos < inner.dates.len() - 1
    }

    pub fn tick(&mut self) {
        let mut inner_mut = self.inner.lock().unwrap();
        inner_mut.pos += 1;
        if inner_mut.pos == inner_mut.dates.len() {
            panic!("Client has ticked past the number of dates");
        }
    }

    //Doesn't change the iteration state, used for clients to setup data using clock
    pub fn peek(&self) -> IntoIter<DateTime> {
        let inner = self.inner.lock().unwrap();
        inner.dates.clone().into_iter()
    }

    /// Get length of clock
    pub fn len(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.dates.len()
    }

    /// Check to see if dates are empty
    pub fn is_empty(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.dates.is_empty()
    }

    pub fn new(dates: Vec<DateTime>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ClockInner { dates, pos: 0 })),
        }
    }
}

/// Used to build [Clock].
pub struct ClockBuilder {
    pub start: DateTime,
    pub end: DateTime,
    pub dates: Vec<DateTime>,
}

impl ClockBuilder {
    const SECS_IN_DAY: i64 = 86_400;

    pub fn build(self) -> Clock {
        Clock::new(self.dates)
    }

    pub fn with_frequency(&self, freq: &Frequency) -> Self {
        match freq {
            Frequency::Daily => {
                let dates: Vec<DateTime> = (i64::from(self.start)
                    ..i64::from(self.end) + ClockBuilder::SECS_IN_DAY)
                    .step_by(ClockBuilder::SECS_IN_DAY as usize)
                    .map(DateTime::from)
                    .collect();
                Self {
                    start: self.start,
                    end: self.end,
                    dates,
                }
            }
            Frequency::Second => {
                let dates: Vec<DateTime> = (i64::from(self.start)..i64::from(self.end) + 1)
                    .map(DateTime::from)
                    .collect();
                Self {
                    start: self.start,
                    end: self.end,
                    dates,
                }
            }
            _ => panic!("Clock frequencies apart from Daily/Second are not supported"),
        }
    }

    //Runs for length given + 1 period
    pub fn with_length_in_seconds(start: impl Into<DateTime>, length_in_seconds: i64) -> Self {
        let start_val = start.into();
        let end = DateTime::from(*start_val + length_in_seconds);
        Self {
            start: start_val,
            end,
            dates: Vec::new(),
        }
    }

    //Runs for length given + 1 period
    pub fn with_length_in_days(start: impl Into<DateTime>, length_in_days: i64) -> Self {
        let start_val = start.into();

        let end = DateTime::from(*start_val + (length_in_days * ClockBuilder::SECS_IN_DAY));
        Self {
            start: start_val,
            end,
            dates: Vec::new(),
        }
    }

    pub fn with_length_in_dates(start: impl Into<DateTime>, end: impl Into<DateTime>) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
            dates: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::types::Frequency;

    use super::ClockBuilder;

    #[test]
    #[should_panic]
    fn test_that_ticking_past_the_length_of_dates_triggers_panic() {
        let mut clock = ClockBuilder::with_length_in_dates(1, 3)
            .with_frequency(&Frequency::Second)
            .build();
        clock.tick();
        clock.tick();
        clock.tick();
    }

    #[test]
    fn test_that_there_isnt_next_when_tick_at_end() {
        let mut clock = ClockBuilder::with_length_in_dates(1, 3)
            .with_frequency(&Frequency::Second)
            .build();
        assert!(clock.has_next());
        clock.tick();

        clock.tick();
        assert!(!clock.has_next());
    }

    #[test]
    fn test_that_clock_created_from_fixed_peeks_correctly() {
        let start = 1;
        let end = start + (3 * 86400);
        let clock = ClockBuilder::with_length_in_dates(start, end)
            .with_frequency(&Frequency::Daily)
            .build();
        let mut dates: Vec<i64> = Vec::new();
        for date in clock.peek() {
            dates.push(i64::from(date));
        }
        assert!(dates == vec![1, 86401, 172801, 259201]);
    }

    #[test]
    fn test_that_clock_created_from_length_peeks_correctly() {
        //Should run for the length given + 1
        let clock = ClockBuilder::with_length_in_seconds(1, 10)
            .with_frequency(&Frequency::Second)
            .build();
        let mut count = 0;
        for _i in clock.peek() {
            count += 1;
        }
        println!("{:?}", count);
        assert!(count == 11);
    }
}
