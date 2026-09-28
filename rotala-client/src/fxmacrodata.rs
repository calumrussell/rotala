pub const API_KEY_HEADER: &str = "X-API-Key";

#[derive(Clone)]
pub struct FXMacroDataClient {
    base_url: String,
    api_key: Option<String>,
}

impl FXMacroDataClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        let api_key = api_key.into();
        Self {
            base_url: "https://api.fxmacrodata.com/v1".to_string(),
            api_key: if api_key.is_empty() {
                None
            } else {
                Some(api_key)
            },
        }
    }

    pub fn data_catalogue(&self, currency: &str) -> String {
        self.url(&format!("/data_catalogue/{}", norm(currency)))
    }
    pub fn announcements(&self, currency: &str, indicator: &str) -> String {
        self.url(&format!("/announcements/{}/{}", norm(currency), indicator))
    }
    pub fn calendar(&self, currency: &str) -> String {
        self.url(&format!("/calendar/{}", norm(currency)))
    }
    pub fn predictions(&self, currency: &str, indicator: &str) -> String {
        self.url(&format!("/predictions/{}/{}", norm(currency), indicator))
    }
    pub fn forex(&self, base: &str, quote: &str) -> String {
        self.url(&format!("/forex/{}/{}", norm(base), norm(quote)))
    }
    pub fn cot(&self, currency: &str) -> String {
        self.url(&format!("/cot/{}", norm(currency)))
    }
    pub fn commodities_latest(&self) -> String {
        self.url("/commodities/latest")
    }
    pub fn commodity(&self, indicator: &str) -> String {
        self.url(&format!("/commodities/{}", indicator))
    }
    pub fn curves(&self, currency: &str) -> String {
        self.url(&format!("/curves/{}", norm(currency)))
    }
    pub fn curve_proxies(&self, currency: &str) -> String {
        self.url(&format!("/curve_proxies/{}", norm(currency)))
    }
    pub fn forward_curves(&self, currency: &str) -> String {
        self.url(&format!("/forward_curves/{}", norm(currency)))
    }
    pub fn market_sessions(&self) -> String {
        self.url("/market_sessions")
    }
    pub fn risk_sentiment(&self) -> String {
        self.url("/risk_sentiment")
    }
    pub fn news(&self, currency: &str) -> String {
        self.url(&format!("/news/{}", norm(currency)))
    }
    pub fn press_releases(&self, currency: &str) -> String {
        self.url(&format!("/press-releases/{}", norm(currency)))
    }

    /// Add `limit` and `offset` to a list endpoint URL. List endpoints return
    /// 20 rows by default and at most 100 per request, newest first; keep
    /// requesting while the response's `pagination.has_more` is true, using
    /// `pagination.next_offset` as the next offset.
    pub fn page(url: &str, limit: u32, offset: u32) -> String {
        let separator = if url.contains('?') { '&' } else { '?' };
        format!(
            "{}{}limit={}&offset={}",
            url,
            separator,
            limit.clamp(1, 100),
            offset
        )
    }

    /// Header to send with each request, or `None` when no key is set.
    pub fn api_key_header(&self) -> Option<(&'static str, &str)> {
        self.api_key.as_deref().map(|key| (API_KEY_HEADER, key))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

impl std::fmt::Debug for FXMacroDataClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FXMacroDataClient")
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

fn norm(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::FXMacroDataClient;

    #[test]
    fn page_adds_limit_and_offset() {
        let client = FXMacroDataClient::new("");
        assert_eq!(
            FXMacroDataClient::page(&client.forex("EUR", "USD"), 100, 200),
            "https://api.fxmacrodata.com/v1/forex/eur/usd?limit=100&offset=200"
        );
        assert_eq!(
            FXMacroDataClient::page("https://example.com/x?start_date=2024-01-01", 500, 0),
            "https://example.com/x?start_date=2024-01-01&limit=100&offset=0"
        );
    }
}
