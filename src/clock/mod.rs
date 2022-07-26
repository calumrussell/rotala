use std::cell::RefCell;
use std::rc::Rc;
use std::vec::IntoIter;

use crate::types::DateTime;

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
    pub fn now(&self) -> DateTime {
        //TODO: can fail at runtime if the client ticks beyond the len
        self.dates[self.pos]
    }

    pub fn has_next(&self) -> bool {
        self.pos < self.dates.len() - 1
    }

    pub fn tick(&mut self) {
        self.pos += 1;
    }

    //Doesn't change the iteration state, used for clients to setup data using clock
    pub fn peek(&self) -> IntoIter<DateTime> {
        self.dates.clone().into_iter()
    }
}

pub struct ClockBuilder {
    pub start: DateTime,
    pub end: DateTime,
}

impl ClockBuilder {
    const SECS_IN_DAY: i64 = 86_400;

    pub fn daily(&self) -> Clock {
        let dates: Vec<DateTime> = (i64::from(self.start)
            ..i64::from(self.end) + ClockBuilder::SECS_IN_DAY)
            .step_by(ClockBuilder::SECS_IN_DAY as usize)
            .map(DateTime::from)
            .collect();
        Rc::new(RefCell::new(ClockInner { dates, pos: 0 }))
    }

    pub fn every(&self) -> Clock {
        let dates: Vec<DateTime> = (i64::from(self.start)..i64::from(self.end) + 1)
            .map(DateTime::from)
            .collect();
        Rc::new(RefCell::new(ClockInner { dates, pos: 0 }))
    }

    pub fn from_length(start: &DateTime, length_in_days: i64) -> Self {
        let end = *start + (length_in_days * ClockBuilder::SECS_IN_DAY);
        Self { start: *start, end }
    }

    pub fn from_fixed(start: DateTime, end: DateTime) -> Self {
        Self { start, end }
    }
}
