use std::collections::HashSet;
use std::io::{Cursor, Write};

use crate::{input::penelope::Penelope, clock::{DateTime, Clock}};

/// Get the data from Binance, build quote from open and close of candle, insert the quotes
/// Penelope and initialize the clock.
fn binance_build() -> (Penelope, Clock) {
    let url =
        "https://data.binance.vision/data/spot/daily/klines/BTCUSDT/1m/BTCUSDT-1m-2022-08-03.zip";
    
    let mut penelope = Penelope::new();
    let mut dates: HashSet<DateTime> = HashSet::new();

    if let Ok(resp) = reqwest::blocking::get(url) {
        if let Ok(contents) = resp.bytes() {
            let mut c = Cursor::new(Vec::new());
            let _res = c.write(&contents);

            if let Ok(mut zip) = zip::ZipArchive::new(c) {
                for i in 0..zip.len() {
                    if let Ok(mut zip_file) = zip.by_index(i) {
                        let mut rdr = csv::Reader::from_reader(&mut zip_file);
                        for row in rdr.records().flatten() {
                            /*
                             * Binance data format:
                             * 1607444700000,          // Open time
                             * "18879.99",             // Open
                             * "18900.00",             // High
                             * "18878.98",             // Low
                             * "18896.13",             // Close (or latest price)
                             * "492.363",              // Volume
                             * 1607444759999,          // Close time
                             * "9302145.66080",        // Quote asset volume
                             * 1874,                   // Number of trades
                             * "385.983",              // Taker buy volume
                             * "7292402.33267",        // Taker buy quote asset volume
                             * "0"                     // Ignore.
                             */
                            let open_date = (row[0].parse::<i64>().unwrap()) / 1000;
                            dates.insert(open_date.into());
                            penelope.add_quotes(
                                row[1].parse::<f64>().unwrap().into(),
                                row[1].parse::<f64>().unwrap().into(),
                                open_date.into(),
                                "BTC"
                            );
 
                            let close_date = (row[6].parse::<i64>().unwrap()) / 1000;
                            dates.insert(close_date.into());
                            penelope.add_quotes(
                                row[4].parse::<f64>().unwrap().into(),
                                row[4].parse::<f64>().unwrap().into(),
                                close_date.into(),
                                "BTC"
                            );
                        }
                    }
                }
            }
        }
    }
    let clock = Clock::from_fixed(Vec::from_iter(dates));
    (penelope, clock)
}