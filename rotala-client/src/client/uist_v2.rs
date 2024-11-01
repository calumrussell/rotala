use anyhow::{Error, Result};
use reqwest;
use rotala::exchange::uist_v2::{Order, OrderId};
use rotala::input::athena::Athena;
use rotala_http::http::uist_v2::{
    AppState, BacktestId, CancelOrderRequest, Client, FetchDepthResponse, FetchQuotesResponse,
    InfoResponse, InitResponse, InsertOrderRequest, ModifyOrderRequest, NowResponse, TickResponse,
    UistV2Error,
};
use std::future::{self, Future};

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

    async fn modify_order(
        &mut self,
        order_id: OrderId,
        quantity_change: f64,
        backtest_id: BacktestId,
    ) -> Result<()> {
        let req = ModifyOrderRequest {
            order_id,
            quantity_change,
        };
        Ok(self
            .client
            .post(self.path.clone() + format!("/backtest/{backtest_id}/modify_order").as_str())
            .json(&req)
            .send()
            .await?
            .json::<()>()
            .await?)
    }

    async fn cancel_order(&mut self, order_id: OrderId, backtest_id: BacktestId) -> Result<()> {
        let req = CancelOrderRequest { order_id };
        Ok(self
            .client
            .post(self.path.clone() + format!("/backtest/{backtest_id}/cancel_order").as_str())
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

    async fn fetch_depth(&mut self, backtest_id: BacktestId) -> Result<FetchDepthResponse> {
        Ok(self
            .client
            .get(self.path.clone() + format!("/backtest/{backtest_id}/fetch_depth").as_str())
            .send()
            .await?
            .json::<FetchDepthResponse>()
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

pub struct TestClient {
    state: AppState,
}

impl Client for TestClient {
    fn init(&mut self, dataset_name: String) -> impl Future<Output = Result<InitResponse>> {
        if let Some(id) = self.state.init(dataset_name) {
            future::ready(Ok(InitResponse { backtest_id: id }))
        } else {
            future::ready(Err(Error::new(UistV2Error::UnknownDataset)))
        }
    }

    fn tick(&mut self, backtest_id: BacktestId) -> impl Future<Output = Result<TickResponse>> {
        if let Some(resp) = self.state.tick(backtest_id) {
            future::ready(Ok(TickResponse {
                modified_orders: resp.3,
                inserted_orders: resp.2,
                executed_trades: resp.1,
                has_next: resp.0,
            }))
        } else {
            future::ready(Err(Error::new(UistV2Error::UnknownBacktest)))
        }
    }

    fn insert_order(
        &mut self,
        order: Order,
        backtest_id: BacktestId,
    ) -> impl Future<Output = Result<()>> {
        if let Some(()) = self.state.insert_order(order, backtest_id) {
            future::ready(Ok(()))
        } else {
            future::ready(Err(Error::new(UistV2Error::UnknownBacktest)))
        }
    }

    fn modify_order(
        &mut self,
        order_id: OrderId,
        quantity_change: f64,
        backtest_id: BacktestId,
    ) -> impl Future<Output = Result<()>> {
        if let Some(()) = self
            .state
            .modify_order(order_id, quantity_change, backtest_id)
        {
            future::ready(Ok(()))
        } else {
            future::ready(Err(Error::new(UistV2Error::UnknownBacktest)))
        }
    }

    fn cancel_order(
        &mut self,
        order_id: OrderId,
        backtest_id: BacktestId,
    ) -> impl Future<Output = Result<()>> {
        if let Some(()) = self.state.cancel_order(order_id, backtest_id) {
            future::ready(Ok(()))
        } else {
            future::ready(Err(Error::new(UistV2Error::UnknownBacktest)))
        }
    }

    fn fetch_quotes(
        &mut self,
        backtest_id: BacktestId,
    ) -> impl Future<Output = Result<FetchQuotesResponse>> {
        if let Some(quotes) = self.state.fetch_quotes(backtest_id) {
            future::ready(Ok(FetchQuotesResponse {
                quotes: quotes.to_owned(),
            }))
        } else {
            future::ready(Err(Error::new(UistV2Error::UnknownBacktest)))
        }
    }

    fn fetch_depth(
        &mut self,
        backtest_id: BacktestId,
    ) -> impl Future<Output = Result<FetchDepthResponse>> {
        if let Some(quotes) = self.state.fetch_depth(backtest_id) {
            future::ready(Ok(FetchDepthResponse { quotes }))
        } else {
            future::ready(Err(Error::new(UistV2Error::UnknownBacktest)))
        }
    }

    fn info(&mut self, backtest_id: BacktestId) -> impl Future<Output = Result<InfoResponse>> {
        if let Some(backtest) = self.state.backtests.get(&backtest_id) {
            future::ready(Ok(InfoResponse {
                version: "v1".to_string(),
                dataset: backtest.dataset_name.clone(),
            }))
        } else {
            future::ready(Err(Error::new(UistV2Error::UnknownBacktest)))
        }
    }

    fn now(&mut self, backtest_id: BacktestId) -> impl Future<Output = Result<NowResponse>> {
        if let Some(backtest) = self.state.backtests.get(&backtest_id) {
            if let Some(dataset) = self.state.datasets.get(&backtest.dataset_name) {
                let now = backtest.date;
                let mut has_next = false;
                if dataset.has_next(backtest.pos) {
                    has_next = true;
                }
                future::ready(Ok(NowResponse { now, has_next }))
            } else {
                future::ready(Err(Error::new(UistV2Error::UnknownDataset)))
            }
        } else {
            future::ready(Err(Error::new(UistV2Error::UnknownBacktest)))
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
