use std::io::{Cursor, Write};

pub struct BinanceKlinesQuote {
    pub open_date: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub close_date: i64,
    pub quote_volume: f64,
    pub trades_number: i64,
    pub taker_buy_volume: f64,
    pub taker_buy_asset_volume: f64,
}

// Building this way isn't performant but this is easier to reason about atm
pub fn get_binance_1m_klines() -> Vec<BinanceKlinesQuote> {
    let url =
        "https://data.binance.vision/data/spot/daily/klines/BTCUSDT/1m/BTCUSDT-1m-2022-08-03.zip";

    let mut result = Vec::new();

    if let Ok(resp) = reqwest::blocking::get(url) {
        if let Ok(contents) = resp.bytes() {
            let mut c = Cursor::new(Vec::new());
            let _res = c.write(&contents);

            if let Ok(mut zip) = zip::ZipArchive::new(c) {
                for i in 0..zip.len() {
                    if let Ok(mut zip_file) = zip.by_index(i) {
                        let mut rdr = csv::Reader::from_reader(&mut zip_file);
                        for row in rdr.records().flatten() {
                            let open_date = (row[0].parse::<i64>().unwrap()) / 1000;
                            let open = row[1].parse::<f64>().unwrap();
                            let high = row[2].parse::<f64>().unwrap();
                            let low = row[3].parse::<f64>().unwrap();
                            let close = row[4].parse::<f64>().unwrap();
                            let volume = row[5].parse::<f64>().unwrap();
                            let close_date = (row[6].parse::<i64>().unwrap()) / 1000;
                            let quote_volume = row[7].parse::<f64>().unwrap();
                            let trades_number = row[8].parse::<i64>().unwrap();
                            let taker_buy_volume = row[9].parse::<f64>().unwrap();
                            let taker_buy_asset_volume = row[10].parse::<f64>().unwrap();
                            result.push(BinanceKlinesQuote {
                                open_date,
                                open,
                                high,
                                low,
                                close,
                                volume,
                                close_date,
                                quote_volume,
                                trades_number,
                                taker_buy_volume,
                                taker_buy_asset_volume,
                            });
                        }
                    }
                }
            }
        }
    }
    result
}