use anyhow::{Error, Result};
use reqwest;
use rotala::exchange::uist_v2::Order;
use rotala::input::athena::Athena;
use rotala_http::http::uist_v2::{
    AppState, BacktestId, Client, DatasetInfoResponse, InfoResponse, InitRequest, InitResponse,
    InsertOrderRequest, TickResponse, UistV2Error,
};
use std::{
    future::{self, Future},
    mem,
};

#[derive(Debug)]
pub struct HttpClient {
    pub path: String,
    pub client: reqwest::Client,
}

impl Client for HttpClient {
    async fn tick(&self, backtest_id: BacktestId) -> Result<TickResponse> {
        Ok(self
            .client
            .get(self.path.clone() + format!("/backtest/{backtest_id}/tick").as_str())
            .send()
            .await?
            .json::<TickResponse>()
            .await?)
    }

    async fn insert_orders(&self, orders: Vec<Order>, backtest_id: BacktestId) -> Result<()> {
        let req = InsertOrderRequest { orders };
        Ok(self
            .client
            .post(self.path.clone() + format!("/backtest/{backtest_id}/insert_orders").as_str())
            .json(&req)
            .send()
            .await?
            .json::<()>()
            .await?)
    }

    async fn init(
        &self,
        dataset_name: String,
        start_date: i64,
        end_date: i64,
        frequency: u64,
    ) -> Result<InitResponse> {
        let req = InitRequest {
            start_date,
            end_date,
            frequency,
        };
        Ok(self
            .client
            .post(self.path.clone() + format!("/init/{dataset_name}").as_str())
            .json(&req)
            .send()
            .await?
            .json::<InitResponse>()
            .await?)
    }

    async fn info(&self, backtest_id: BacktestId) -> Result<InfoResponse> {
        Ok(self
            .client
            .get(self.path.clone() + format!("/backtest/{backtest_id}/info").as_str())
            .send()
            .await?
            .json::<InfoResponse>()
            .await?)
    }

    async fn dataset_info(&self, dataset_name: String) -> Result<DatasetInfoResponse> {
        Ok(self
            .client
            .get(self.path.clone() + format!("/dataset/{dataset_name}/info").as_str())
            .send()
            .await?
            .json::<DatasetInfoResponse>()
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

pub struct TestClient {
    state: AppState,
}

impl Client for TestClient {
    fn init(
        &self,
        dataset_name: String,
        start_date: i64,
        end_date: i64,
        frequency: u64,
    ) -> impl Future<Output = Result<InitResponse>> {
        if let Some((backtest_id, bbo, depth)) =
            self.state
                .init(dataset_name, start_date, end_date, frequency)
        {
            future::ready(Ok(InitResponse {
                backtest_id,
                bbo,
                depth,
            }))
        } else {
            future::ready(Err(Error::new(UistV2Error::UnknownDataset)))
        }
    }

    fn tick(&self, backtest_id: BacktestId) -> impl Future<Output = Result<TickResponse>> {
        if let Some(resp) = self.state.tick(backtest_id) {
            future::ready(Ok(TickResponse {
                depth: resp.4,
                bbo: resp.3,
                inserted_orders: resp.2,
                executed_orders: resp.1,
                has_next: resp.0,
            }))
        } else {
            future::ready(Err(Error::new(UistV2Error::UnknownBacktest)))
        }
    }

    fn insert_orders(
        &self,
        mut orders: Vec<Order>,
        backtest_id: BacktestId,
    ) -> impl Future<Output = Result<()>> {
        let take_orders = mem::take(&mut orders);
        if let Some(()) = self.state.insert_orders(take_orders, backtest_id) {
            future::ready(Ok(()))
        } else {
            future::ready(Err(Error::new(UistV2Error::UnknownBacktest)))
        }
    }

    fn info(&self, backtest_id: BacktestId) -> impl Future<Output = Result<InfoResponse>> {
        if let Some(backtest) = self.state.backtests.get(&backtest_id) {
            future::ready(Ok(InfoResponse {
                version: "v1".to_string(),
                dataset: backtest.dataset_name.clone(),
            }))
        } else {
            future::ready(Err(Error::new(UistV2Error::UnknownBacktest)))
        }
    }

    fn dataset_info(
        &self,
        dataset_name: String,
    ) -> impl Future<Output = Result<DatasetInfoResponse>> {
        if let Some(dataset) = self.state.dataset_info(&dataset_name) {
            future::ready(Ok(DatasetInfoResponse {
                start_date: dataset.0,
                end_date: dataset.1,
            }))
        } else {
            future::ready(Err(Error::new(UistV2Error::UnknownDataset)))
        }
    }
}

impl TestClient {
    pub fn single(name: &str, data: Athena) -> Self {
        Self {
            state: AppState::single(name, data),
        }
    }
}
