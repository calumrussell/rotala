extern crate csv;

use alator::broker::Quote;
use alator::data::universe::DefinedUniverse;
use alator::data::universe::StaticUniverse;
use csv::StringRecord;
use itertools::Itertools;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct Row {
    symbol: String,
    order_type: String,
    price: f64,
    date: u64,
}

fn read_csv() -> Result<Vec<Row>, Box<csv::Error>> {
    let file_path = Path::new("./tests/longer_sample.csv");
    let mut rdr = csv::Reader::from_path(file_path)?;
    let header = StringRecord::from(vec!["symbol", "order_type", "price", "date"]);

    let mut tempbuff: Vec<Row> = Vec::new();
    for result in rdr.records() {
        let row: Row = result?.deserialize(Some(&header))?;
        tempbuff.push(row);
    }
    Ok(tempbuff)
}

pub fn build_csv(res: &mut HashMap<i64, Vec<Quote>>) -> Result<bool, Box<csv::Error>> {
    let csv_contents = read_csv()?;

    fn grouper(i: &Row) -> String {
        let mut s = String::new();
        s.push_str(i.symbol.as_str());
        s.push_str(" - ");
        s.push_str(i.date.to_string().as_str());
        s
    }

    for (_key, group) in &csv_contents.into_iter().group_by(grouper) {
        let mut bid = 0.0;
        let mut ask = 0.0;
        let mut date = 0 as i64;
        let mut symbol = String::new();

        for price in group {
            if price.order_type == "bid" {
                bid = price.price;
            } else if price.order_type == "ask" {
                ask = price.price;
            }
            date = price.date as i64;
            symbol = price.symbol;
        }

        let quote = Quote {
            bid,
            ask,
            date,
            symbol,
        };
        if res.contains_key(&date) {
            let mut temp = res.get(&date).unwrap().clone();
            temp.push(quote);
            res.insert(date, temp);
        } else {
            let mut temp: Vec<Quote> = Vec::new();
            temp.push(quote);
            res.insert(date, temp);
        }
    }
    Ok(true)
}

pub fn get_universe_weights() -> (StaticUniverse, HashMap<String, f64>) {
    let uni = StaticUniverse::new(vec![
        "ABC", "BCD", "CDE", "DEF", "EFG", "FGH", "GHI", "HIJ", "IJK", "JKL", "KLM", "LMN", "MNO",
        "NOP",
    ]);

    let psize = 1.0 / uni.get_assets().len() as f64;
    let mut weights: HashMap<String, f64> = HashMap::new();
    for a in uni.get_assets() {
        weights.insert(a.clone(), psize);
    }
    (uni, weights)
}
