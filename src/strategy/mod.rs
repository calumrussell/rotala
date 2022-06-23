/* A Strategy wraps around the broker and portfolio, the idea
is to move most of the functionality into a trading strategy
and organize calls to the rest of the system through that.

One key point is that the Strategy should only be aware of an
overall portfolio, and not aware of how the portfolio executes
changes with the broker.
*/

use crate::{
    data::{CashValue, DateTime},
    perf::PerfStruct,
};

pub mod fixedweight;
pub mod randomfake;
pub mod staticweight;

pub trait Strategy {
    fn run(&mut self) -> CashValue;
    fn set_date(&mut self, date: &DateTime);
    fn init(&mut self, initial_cash: &CashValue);
    fn get_perf(&self) -> PerfStruct;
}
