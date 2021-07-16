extern crate csv;

use alator::data::universe::StaticUniverse;
use alator::types::Quote;
use alator::types::StockQuote;
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

pub fn build_csv(
    res: &mut HashMap<i64, Vec<alator::types::StockQuote>>,
) -> Result<bool, Box<csv::Error>> {

    let csv_contents = read_csv()?;

    fn grouper(i: &Row) -> String {
        let mut s = String::new();
        s.push_str(i.symbol.as_str());
        s.push_str(" - ");
        s.push_str(i.date.to_string().as_str());
        s
    }

    for (key, group) in &csv_contents.into_iter().group_by(grouper) {
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

        let quote = Quote { bid, ask, date };
        let sq = StockQuote { symbol, quote };

        if res.contains_key(&date) {
            let mut temp = res.get(&date).unwrap().clone();
            temp.push(sq);
            res.insert(date, temp);
        } else {
            let mut temp: Vec<StockQuote> = Vec::new();
            temp.push(sq);
            res.insert(date, temp);
        }
    }
    Ok(true)
}

pub fn get_universe_weights() -> (Box<StaticUniverse>, HashMap<String, f64>) {
    let uni = Box::new(StaticUniverse::new(vec![
        "ABC", "BCD", "CDE", "DEF", "EFG", "FGH", "GHI", "HIJ", "IJK", "JKL", "KLM", "LMN", "MNO",
        "NOP",
    ]));
    let mut weights: HashMap<String, f64> = HashMap::new();
    weights.insert(String::from("ABC"), 0.06);
    weights.insert(String::from("BCD"), 0.06);
    weights.insert(String::from("CDE"), 0.06);
    weights.insert(String::from("DEF"), 0.06);
    weights.insert(String::from("EFG"), 0.06);
    weights.insert(String::from("FGH"), 0.06);
    weights.insert(String::from("GHI"), 0.06);
    weights.insert(String::from("HIJ"), 0.06);
    weights.insert(String::from("IJK"), 0.06);
    (uni, weights)
}
