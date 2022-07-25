use std::vec::IntoIter;
use std::cell::RefCell;
use std::rc::Rc;

use crate::types::DateTime;

pub type Clock = Rc<RefCell<ClockInner>>;

#[derive(Debug)]
pub struct ClockInner {
    pos: usize,
    dates: Vec<DateTime>,
}

impl ClockInner {
    pub fn now(&self) -> DateTime {
        self.dates[self.pos]
    }

    pub fn has_next(&self) -> bool {
        self.pos < self.dates.len() -1 
    }

    pub fn tick(&mut self) {
        self.pos += 1;
    }

    //Note that this doesn't change the iteration state, used for clients to setup data using clock
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
        let dates: Vec<DateTime> = (i64::from(self.start)..i64::from(self.end)+ClockBuilder::SECS_IN_DAY)
            .step_by(ClockBuilder::SECS_IN_DAY as usize)
            .map(|date_int: i64| DateTime::from(date_int))
            .collect();
        Rc::new(RefCell::new(ClockInner {
            dates,
            pos: 0
        }))
    }

    pub fn every(&self) -> Clock {
        let dates: Vec<DateTime> = (i64::from(self.start)..i64::from(self.end)+1) 
            .map(|date_int: i64| DateTime::from(date_int))
            .collect();
        Rc::new(RefCell::new(ClockInner {
            dates,
            pos: 0
        }))
    }

    pub fn from_length(start: &DateTime, length_in_days: i64) -> Self {
        let end = *start + (length_in_days * ClockBuilder::SECS_IN_DAY);
        Self {
            start: start.clone(),
            end,
        }
    }

    pub fn from_fixed(start: DateTime, end: DateTime) -> Self {
        Self {
            start,
            end,
        }
    }
}
