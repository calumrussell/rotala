//! Synchronizes time across components

use std::ops::Deref;
use std::sync::Arc;
use std::sync::Mutex;
use std::vec::IntoIter;
use time::{format_description, Date, OffsetDateTime};

///The frequency of a process.
#[derive(Clone, Debug)]
pub enum Frequency {
    Second,
    Daily,
    Fixed,
}

impl From<Frequency> for u8 {
    fn from(freq: Frequency) -> Self {
        match freq {
            Frequency::Second => 0,
            Frequency::Daily => 1,
            Frequency::Fixed => 3,
        }
    }
}

pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl From<time::Weekday> for Weekday {
    fn from(v: time::Weekday) -> Self {
        match v {
            time::Weekday::Monday => Weekday::Monday,
            time::Weekday::Tuesday => Weekday::Tuesday,
            time::Weekday::Wednesday => Weekday::Wednesday,
            time::Weekday::Thursday => Weekday::Thursday,
            time::Weekday::Friday => Weekday::Friday,
            time::Weekday::Saturday => Weekday::Saturday,
            time::Weekday::Sunday => Weekday::Sunday,
        }
    }
}

pub enum Month {
    January,
    February,
    March,
    April,
    May,
    June,
    July,
    August,
    September,
    October,
    November,
    December,
}

impl From<time::Month> for Month {
    fn from(v: time::Month) -> Self {
        match v {
            time::Month::January => Month::January,
            time::Month::February => Month::February,
            time::Month::March => Month::March,
            time::Month::April => Month::April,
            time::Month::May => Month::May,
            time::Month::June => Month::June,
            time::Month::July => Month::July,
            time::Month::August => Month::August,
            time::Month::September => Month::September,
            time::Month::October => Month::October,
            time::Month::November => Month::November,
            time::Month::December => Month::December,
        }
    }
}

impl From<Month> for u8 {
    fn from(v: Month) -> Self {
        match v {
            Month::January => 1,
            Month::February => 2,
            Month::March => 3,
            Month::April => 4,
            Month::May => 5,
            Month::June => 6,
            Month::July => 7,
            Month::August => 8,
            Month::September => 9,
            Month::October => 10,
            Month::November => 11,
            Month::December => 12,
        }
    }
}

///[DateTime] is a wrapper around the epoch time as i64. This type also functions as a wrapper
///around the time package which offers some of the more useful datetime functionality that is
///required in the schedule module.
//The internal representation with the time package should remain hidden from clients. Whilst this
//results in some duplication of the API, this retains the option to get rid of the dependency on
//time or change individual functions later.
#[derive(Clone, Debug, Hash, Eq, PartialEq, PartialOrd, Copy, Ord)]
pub struct DateTime(i64);

impl DateTime {
    pub fn day(&self) -> u8 {
        let date: OffsetDateTime = (*self).into();
        date.day()
    }

    pub fn weekday(&self) -> Weekday {
        let date: OffsetDateTime = (*self).into();
        date.weekday().into()
    }

    pub fn month(&self) -> Month {
        let date: OffsetDateTime = (*self).into();
        date.month().into()
    }

    pub fn from_date_string(val: &str, date_fmt: &str) -> Self {
        let format = format_description::parse(date_fmt).unwrap();
        let parsed_date = Date::parse(val, &format).unwrap();
        let parsed_time = parsed_date.with_time(time::macros::time!(09:00));
        Self::from(parsed_time.assume_utc().unix_timestamp())
    }
}

impl Deref for DateTime {
    type Target = i64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<OffsetDateTime> for DateTime {
    fn from(value: OffsetDateTime) -> Self {
        value.unix_timestamp().into()
    }
}

impl From<DateTime> for OffsetDateTime {
    fn from(v: DateTime) -> Self {
        if let Ok(date) = OffsetDateTime::from_unix_timestamp(i64::from(v)) {
            date
        } else {
            panic!("Tried to convert non-date value");
        }
    }
}

impl From<DateTime> for i64 {
    fn from(v: DateTime) -> Self {
        v.0
    }
}

impl From<i64> for DateTime {
    fn from(v: i64) -> Self {
        DateTime(v)
    }
}

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
    frequency: Frequency,
}

impl Clone for Clock {
    fn clone(&self) -> Self {
        Clock {
            inner: Arc::clone(&self.inner),
            frequency: self.frequency.clone(),
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

    // Doesn't change the iteration state, used for clients to setup data using clock
    pub fn peek(&self) -> IntoIter<DateTime> {
        let inner = self.inner.lock().unwrap();
        inner.dates.clone().into_iter()
    }

    /// Get length of clock
    pub fn len(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.dates.len()
    }

    pub fn frequency(&self) -> &Frequency {
        &self.frequency
    }

    /// Check to see if dates are empty
    pub fn is_empty(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.dates.is_empty()
    }

    pub fn from_fixed(dates: Vec<DateTime>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ClockInner { dates, pos: 0 })),
            frequency: Frequency::Fixed,
        }
    }

    pub fn new(dates: Vec<DateTime>, frequency: Frequency) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ClockInner { dates, pos: 0 })),
            frequency,
        }
    }
}

/// Used to build [Clock].
pub struct ClockBuilder {
    pub start: DateTime,
    pub end: DateTime,
    pub dates: Vec<DateTime>,
    pub frequency: Frequency,
}

impl ClockBuilder {
    const SECS_IN_DAY: i64 = 86_400;

    pub fn build(self) -> Clock {
        Clock::new(self.dates, self.frequency)
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
                    frequency: Frequency::Daily,
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
                    frequency: Frequency::Second,
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
            frequency: Frequency::Second,
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
            frequency: Frequency::Daily,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ClockBuilder, Frequency};

    #[test]
    #[should_panic]
    fn test_that_ticking_past_the_length_of_dates_triggers_panic() {
        let mut clock = ClockBuilder::with_length_in_seconds(1, 2)
            .with_frequency(&Frequency::Second)
            .build();
        clock.tick();
        clock.tick();
        clock.tick();
    }

    #[test]
    fn test_that_there_isnt_next_when_tick_at_end() {
        let mut clock = ClockBuilder::with_length_in_seconds(1, 2)
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
        let clock = ClockBuilder::with_length_in_days(start, 3)
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
