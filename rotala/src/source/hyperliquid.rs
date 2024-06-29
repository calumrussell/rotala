use std::{collections::HashMap, fs::read_to_string};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::from_str;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Level {
    pub px: String,
    pub sz: String,
    pub n: i8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PointInTime {
    pub coin: String,
    pub time: u64,
    pub levels: Vec<Vec<Level>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Data {
    pub channel: String,
    pub data: PointInTime,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct L2Book {
    pub time: String,
    pub ver_num: u64,
    pub raw: Data,
}

pub fn get_hyperliquid_l2(path: &Path) -> HashMap<u64, L2Book> {
    let mut result = HashMap::new();
    if let Ok(file_contents) = read_to_string(path) {
        for line in file_contents.split('\n') {
            if line.is_empty() {
                continue;
            }

            let val: L2Book = from_str(line).unwrap();
            let time = val.raw.data.time;
            result.insert(time, val);
        }
    }
    result
}

