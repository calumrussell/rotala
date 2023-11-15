use std::collections::HashMap;

#[derive(Debug)]
pub struct DefaultPriceSource {
    inner: HashMap<i64, HashMap<String, crate::Quote>>,
}

impl DefaultPriceSource {
    pub fn get_quote(&self, date: &i64, symbol: &str) -> Option<&crate::Quote> {
        if let Some(date_row) = self.inner.get(date) {
            if let Some(quote) = date_row.get(symbol) {
                return Some(quote);
            }
        }
        None
    }

    pub fn get_quotes(&self, date: &i64) -> Option<Vec<crate::Quote>> {
        if let Some(date_row) = self.inner.get(date) {
            return Some(date_row.values().cloned().collect());
        }
        None
    }

    pub fn add_quotes(&mut self, bid: f64, ask: f64, date: i64, symbol: impl Into<String> + Clone) {
        let quote = crate::Quote {
            bid,
            ask,
            date,
            symbol: symbol.clone().into(),
        };

        if let Some(date_row) = self.inner.get_mut(&date) {
            date_row.insert(symbol.into(), quote);
        } else {
            let mut date_row = HashMap::new();
            date_row.insert(symbol.into(), quote);
            self.inner.insert(date, date_row);
        }
    }

    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn from_hashmap(inner: HashMap<i64, HashMap<String, crate::Quote>>) -> Self {
        Self { inner }
    }
}

impl Default for DefaultPriceSource {
    fn default() -> Self {
        Self::new()
    }
}
