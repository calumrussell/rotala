use anyhow::Result;
use reqwest;
use rotala::exchange::uist_v2::Order;
use rotala_http::http::uist_v2::{
    BacktestId, Client, FetchQuotesResponse, InfoResponse, InitResponse, InsertOrderRequest,
    NowResponse, TickResponse,
};

#[derive(Debug)]
pub struct HttpClient {
    pub path: String,
    pub client: reqwest::Client,
}

impl Client for HttpClient {
    async fn tick(&mut self, backtest_id: BacktestId) -> Result<TickResponse> {
        Ok(self
            .client
            .get(self.path.clone() + format!("/backtest/{backtest_id}/tick").as_str())
            .send()
            .await?
            .json::<TickResponse>()
            .await?)
    }

    async fn insert_order(&mut self, order: Order, backtest_id: BacktestId) -> Result<()> {
        let req = InsertOrderRequest { order };
        Ok(self
            .client
            .post(self.path.clone() + format!("/backtest/{backtest_id}/insert_order").as_str())
            .json(&req)
            .send()
            .await?
            .json::<()>()
            .await?)
    }

    async fn fetch_quotes(&mut self, backtest_id: BacktestId) -> Result<FetchQuotesResponse> {
        Ok(self
            .client
            .get(self.path.clone() + format!("/backtest/{backtest_id}/fetch_quotes").as_str())
            .send()
            .await?
            .json::<FetchQuotesResponse>()
            .await?)
    }

    async fn init(&mut self, dataset_name: String) -> Result<InitResponse> {
        Ok(self
            .client
            .get(self.path.clone() + format!("/init/{dataset_name}").as_str())
            .send()
            .await?
            .json::<InitResponse>()
            .await?)
    }

    async fn info(&mut self, backtest_id: BacktestId) -> Result<InfoResponse> {
        Ok(self
            .client
            .get(self.path.clone() + format!("/backtest/{backtest_id}/info").as_str())
            .send()
            .await?
            .json::<InfoResponse>()
            .await?)
    }

    async fn now(&mut self, backtest_id: BacktestId) -> Result<NowResponse> {
        Ok(self
            .client
            .get(self.path.clone() + format!("/backtest/{backtest_id}/now").as_str())
            .send()
            .await?
            .json::<NowResponse>()
            .await?)
    }
}

impl HttpClient {
    pub fn new(path: String) -> Self {
        Self {
            path,
            client: reqwest::Client::new(),
        }
    }
}
