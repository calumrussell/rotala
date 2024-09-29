use std::path::Path;
use std::{collections::HashMap, fs::read_to_string};

use serde::{Deserialize, Serialize};
use serde_json::from_str;

use crate::input::athena::{Depth, Level};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HyperLiquidLevel {
    pub px: String,
    pub sz: String,
    pub n: i8,
}

impl From<HyperLiquidLevel> for Level {
    fn from(value: HyperLiquidLevel) -> Self {
        Self {
            price: value.px.parse().unwrap(),
            size: value.sz.parse().unwrap(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PointInTime {
    pub coin: String,
    pub time: u64,
    pub levels: Vec<Vec<HyperLiquidLevel>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PointInTimeWrapper {
    pub channel: String,
    pub data: PointInTime,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct L2Book {
    pub time: String,
    pub ver_num: u64,
    pub raw: PointInTimeWrapper,
}

impl From<L2Book> for Depth {
    fn from(value: L2Book) -> Depth {
        let date = value.raw.data.time as i64;
        let symbol = value.raw.data.coin;

        let mut bids_depth: Vec<Level> = Vec::new();
        let mut asks_depth: Vec<Level> = Vec::new();

        if let Some(bids) = value.raw.data.levels.first() {
            let bids_depth_tmp: Vec<Level> =
                bids.iter().map(|v| -> Level { v.clone().into() }).collect();
            bids_depth.extend(bids_depth_tmp);
        }

        if let Some(asks) = value.raw.data.levels.get(1) {
            let asks_depth_tmp: Vec<Level> =
                asks.iter().map(|v| -> Level { v.clone().into() }).collect();
            asks_depth.extend(asks_depth_tmp);
        }

        Depth {
            bids: bids_depth,
            asks: asks_depth,
            date,
            symbol,
        }
    }
}

pub fn get_hyperliquid_l2(path: &Path) -> HashMap<u64, L2Book> {
    let mut result = HashMap::new();

    if let Ok(dir_contents) = path.read_dir() {
        for coin in dir_contents.flatten() {
            if let Ok(coin_dir_contents) = coin.path().read_dir() {
                for period in coin_dir_contents.flatten() {

                    if let Ok(file_contents) = read_to_string(period.path()) {
                        for line in file_contents.split('\n') {
                            if line.is_empty() {
                                continue;
                            }

                            let val: L2Book = from_str(line).unwrap();
                            let time = val.raw.data.time;
                            result.insert(time, val);
                        }
                    }
                }
            }
        }
    }
    result
}
