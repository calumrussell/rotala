use crate::types::{DateTime, Weekday};

///`TradingSchedule` should be used within `Strategy` implementations to control when a rebalancing
///should run. This is a helper to the creation of strategies, and is not required by any other
///component. Despite the name, which implies use for rebalancing only, it can also be used to
///indicate when `Strategy` data needs to be updated or similar.
//TODO: make this more generic, there is no reason why it has to only concern a rebalancing
//schedule
pub trait TradingSchedule {
    fn should_trade(date: &DateTime) -> bool;
}

pub struct DefaultTradingSchedule;

impl TradingSchedule for DefaultTradingSchedule {
    fn should_trade(_date: &DateTime) -> bool {
        true
    }
}

pub struct LastBusinessDayTradingSchedule;

impl TradingSchedule for LastBusinessDayTradingSchedule {
    fn should_trade(date: &DateTime) -> bool {
        if (*date).day() < (28 - 7) {
            return false;
        }

        match (*date).weekday() {
            Weekday::Saturday | Weekday::Sunday => {
                return false;
            }
            _ => (),
        }

        //Only need to check up to four dates as we are checking for weekends. The day should
        //either be a weekend or a day with a different month. If either of these things is false
        //then we return false.
        //The day of the new month is not necessarily the first day.
        //
        for i in 1..4 {
            let seconds_in_day = 86400;
            let offset_time = DateTime::from(*date.clone() + (i * seconds_in_day));
            match offset_time.weekday() {
                Weekday::Saturday | Weekday::Sunday => continue,
                _ => {
                    if offset_time.month() as u8 == (*date).month() as u8 {
                        return false;
                    } else {
                        continue;
                    }
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {

    use super::{LastBusinessDayTradingSchedule, TradingSchedule};

    #[test]
    fn test_that_schedule_returns_true_for_last_day_of_month() {
        // Date 30/09/21 - 17:00:0000
        assert!(LastBusinessDayTradingSchedule::should_trade(
            &1633021200.into()
        ));
        // Date 29/10/21 - 17:00:0000
        assert!(LastBusinessDayTradingSchedule::should_trade(
            &1635526800.into()
        ));
    }

    #[test]
    fn test_that_schedule_returns_false_for_non_last_day_of_month() {
        // Date 1/11/21 - 9:00:0000
        assert!(!LastBusinessDayTradingSchedule::should_trade(
            &1635757200.into()
        ));
        // Date 12/11/21 - 17:00:0000
        assert!(!LastBusinessDayTradingSchedule::should_trade(
            &1636736400.into()
        ));
        //Date 31/10/21 - 9:00:0000
        assert!(!LastBusinessDayTradingSchedule::should_trade(
            &1635670800.into()
        ));
        //Date 22/1/21 - 9:00:0000
        assert!(!LastBusinessDayTradingSchedule::should_trade(
            &1611306000.into()
        ));
    }
}
