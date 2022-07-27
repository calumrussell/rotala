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
        //This cannot trigger an error because the error will be thrown when the client ticks to an
        //invalid position
        self.dates[self.pos]
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

#[cfg(test)]
mod tests {
    use super::ClockBuilder;

    #[test]
    #[should_panic]
    fn test_that_ticking_past_the_length_of_dates_triggers_panic() {
        let clock = ClockBuilder::from_fixed(1.into(), 3.into()).every();
        clock.borrow_mut().tick();
        clock.borrow_mut().tick();
        clock.borrow_mut().tick();
    }

    #[test]
    fn test_that_there_isnt_next_when_tick_at_end() {
        let clock = ClockBuilder::from_fixed(1.into(), 3.into()).every();
        assert!(clock.borrow().has_next() == true);
        clock.borrow_mut().tick();

        clock.borrow_mut().tick();
        assert!(clock.borrow().has_next() == false);
    }
}
