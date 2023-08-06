use std::cell::RefCell;
use std::rc::Rc;
use std::vec::IntoIter;

use crate::types::{DateTime, Frequency};

///Clock is a reference to the internal clock used be all components that have a dependency on the
///date within the backtest.
///
///Previous implementations did not use a shared clock which result in runtime errors and
///complexity when some parts of the system updated their time but others did not. By creating a
///shared reference, we significantly reduce the scope for unexpected behaviour due to
///inadvertently incorrect sequencing of operations. An added benefit is that this significantly
///simplifies the interface for data queries so that live-trading would be possible.
pub type Clock = Rc<RefCell<ClockInner>>;

#[derive(Debug)]
pub struct ClockInner {
    //We have a position and Vec because we should be able to return an iterator without changing
    //the state of the Clock
    pos: usize,
    dates: Vec<DateTime>,
}

impl ClockInner {
    //TODO: this should return an Option<&DateTime>
    pub fn now(&self) -> DateTime {
        //This cannot trigger an error because the error will be thrown when the client ticks to an
        //invalid position
        self.dates.get(self.pos).unwrap().clone()
    }

    pub fn has_next(&self) -> bool {
        self.pos < self.dates.len() - 1
    }

    pub fn tick(&mut self) {
        self.pos += 1;
        if self.pos == self.dates.len() {
            panic!("Client has ticked past the number of dates");
        }
    }

    //Doesn't change the iteration state, used for clients to setup data using clock
    pub fn peek(&self) -> IntoIter<DateTime> {
        self.dates.clone().into_iter()
    }

    /// Get length of clock
    pub fn len(&self) -> usize {
        self.dates.len()
    }

    /// Check to see if dates are empty
    pub fn is_empty(&self) -> bool {
        self.dates.is_empty()
    }
}

pub struct ClockBuilder {
    pub start: DateTime,
    pub end: DateTime,
    pub dates: Vec<DateTime>,
}

impl ClockBuilder {
    const SECS_IN_DAY: i64 = 86_400;

    pub fn build(self) -> Clock {
        Rc::new(RefCell::new(ClockInner {
            dates: self.dates,
            pos: 0,
        }))
    }

    pub fn with_frequency(&self, freq: &Frequency) -> Self {
        match freq {
            Frequency::Daily => {
                let dates: Vec<DateTime> = (i64::from(self.start.clone())
                    ..i64::from(self.end.clone()) + ClockBuilder::SECS_IN_DAY)
                    .step_by(ClockBuilder::SECS_IN_DAY as usize)
                    .map(DateTime::from)
                    .collect();
                Self {
                    start: self.start.clone(),
                    end: self.end.clone(),
                    dates,
                }
            }
            Frequency::Second => {
                let dates: Vec<DateTime> = (i64::from(self.start.clone())
                    ..i64::from(self.end.clone()) + 1)
                    .map(DateTime::from)
                    .collect();
                Self {
                    start: self.start.clone(),
                    end: self.end.clone(),
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
        let clock = ClockBuilder::with_length_in_dates(1, 3)
            .with_frequency(&Frequency::Second)
            .build();
        clock.borrow_mut().tick();
        clock.borrow_mut().tick();
        clock.borrow_mut().tick();
    }

    #[test]
    fn test_that_there_isnt_next_when_tick_at_end() {
        let clock = ClockBuilder::with_length_in_dates(1, 3)
            .with_frequency(&Frequency::Second)
            .build();
        assert!(clock.borrow().has_next());
        clock.borrow_mut().tick();

        clock.borrow_mut().tick();
        assert!(!clock.borrow().has_next());
    }

    #[test]
    fn test_that_clock_created_from_fixed_peeks_correctly() {
        let start = 1;
        let end = start + (3 * 86400);
        let clock = ClockBuilder::with_length_in_dates(start, end)
            .with_frequency(&Frequency::Daily)
            .build();
        let mut dates: Vec<i64> = Vec::new();
        for date in clock.borrow().peek() {
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
        for _i in clock.borrow().peek() {
            count += 1;
        }
        println!("{:?}", count);
        assert!(count == 11);
    }
}
