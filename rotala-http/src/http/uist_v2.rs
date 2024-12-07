use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::sync::atomic::AtomicU64;

use anyhow::Result;
use dashmap::try_result::TryResult;
use dashmap::DashMap;
use rotala::input::minerva::Minerva;
use serde::{Deserialize, Serialize};

use rotala::exchange::uist_v2::{InnerOrder, Order, OrderId, OrderResult, UistV2};
use rotala::source::hyperliquid::{DateDepth, DateTrade};

pub type BacktestId = u64;
pub type TickResponseType = (
    bool,
    Vec<OrderResult>,
    Vec<InnerOrder>,
    DateDepth,
    i64,
    DateTrade,
);

pub struct BacktestState {
    pub id: BacktestId,
    pub start_date: i64,
    pub curr_date: i64,
    pub frequency: u64,
    pub end_date: i64,
    pub exchange: UistV2,
    pub dataset_name: String,
}

pub struct AppState {
    pub backtests: DashMap<BacktestId, BacktestState>,
    pub last: AtomicU64,
    pub datasets: HashMap<String, Minerva>,
}

impl AppState {
    pub fn create(datasets: &mut HashMap<String, Minerva>) -> Self {
        Self {
            backtests: DashMap::new(),
            last: AtomicU64::new(0),
            datasets: std::mem::take(datasets),
        }
    }

    pub fn single(name: &str, data: Minerva) -> Self {
        let mut datasets = HashMap::new();
        datasets.insert(name.into(), data);

        Self {
            backtests: DashMap::new(),
            last: AtomicU64::new(1),
            datasets,
        }
    }

    pub async fn tick(&self, backtest_id: BacktestId) -> Option<TickResponseType> {
        if let TryResult::Present(mut backtest) = self.backtests.try_get_mut(&backtest_id) {
            if let Some(dataset) = self.datasets.get(&backtest.dataset_name) {
                let mut executed_orders = Vec::new();
                let mut inserted_orders = Vec::new();

                let curr_date = backtest.curr_date;

                let quotes = dataset.get_depth_between(backtest.curr_date - backtest.frequency as i64..backtest.curr_date).await;
                if let Some((_date, date_quotes)) = quotes.last_key_value() {
                    let mut res = backtest.exchange.tick(date_quotes, curr_date);

                    executed_orders = std::mem::take(&mut res.0);
                    inserted_orders = std::mem::take(&mut res.1);
                }

                let new_date = backtest.curr_date + backtest.frequency as i64;
                if new_date >= backtest.end_date {
                    return Some((
                        false,
                        Vec::new(),
                        Vec::new(),
                        BTreeMap::new(),
                        new_date,
                        BTreeMap::new(),
                    ));
                } else {
                    let depth = dataset.get_depth_between(backtest.curr_date..new_date).await;
                    let mut last_depth = BTreeMap::default();
                    if let Some((_date, last_quotes)) = depth.last_key_value() {
                        //TODO: not great, as state is stored in DB it isn't clear why we need
                        //clone
                        last_depth = last_quotes.clone();
                    }

                    let trades = dataset.get_trades(backtest.curr_date..new_date).await;
                    backtest.curr_date = new_date;
                    return Some((true, executed_orders, inserted_orders, last_depth, new_date, trades));
                }
            }
        }
        None
    }

    pub async fn init(
        &self,
        dataset_name: String,
        start_date: i64,
        end_date: i64,
        frequency: u64,
    ) -> Option<(BacktestId, DateDepth)> {
        if let Some(dataset) = self.datasets.get(&dataset_name) {
            let curr_id = self.last.load(std::sync::atomic::Ordering::SeqCst);
            let exchange = UistV2::new();

            let dataset_date_bounds = dataset.get_date_bounds().await.unwrap();
            let end_date_backtest = if dataset_date_bounds.1 > end_date {
                end_date
            } else {
                dataset_date_bounds.1
            };

            let backtest = BacktestState {
                id: curr_id,
                start_date,
                curr_date: start_date,
                end_date: end_date_backtest,
                frequency,
                exchange,
                dataset_name,
            };

            let new_id = curr_id + 1;
            //Attempt to increment the counter, if this is successful then we create new backtest
            if let Ok(res) = self.last.compare_exchange(
                curr_id,
                new_id,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            ) {
                if res == curr_id {
                    self.backtests.insert(curr_id, backtest);

                    let depth = dataset.get_depth_between(start_date - frequency as i64..start_date).await;
                    let mut last_depth = BTreeMap::default();
                    if let Some((_date, last_value)) = depth.last_key_value() {
                        //TODO: isn't clear why clone is required here, same as above somewhere
                        last_depth = last_value.clone();
                    }
                    return Some((curr_id, last_depth));
                }
            }
        }
        None
    }

    pub fn insert_orders(&self, orders: Vec<Order>, backtest_id: BacktestId) -> Option<()> {
        if let TryResult::Present(mut backtest) = self.backtests.try_get_mut(&backtest_id) {
            backtest.exchange.insert_orders(orders);
            return Some(());
        }
        None
    }

    pub async fn dataset_info(&self, dataset_name: &str) -> Option<(i64, i64)> {
        if let Some(dataset) = self.datasets.get(dataset_name) {
            return dataset.get_date_bounds().await;
        }
        None
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TickResponse {
    pub has_next: bool,
    pub executed_orders: Vec<OrderResult>,
    pub inserted_orders: Vec<InnerOrder>,
    pub depth: DateDepth,
    pub taker_trades: DateTrade,
    pub now: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InsertOrderRequest {
    pub orders: Vec<Order>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InitRequest {
    pub start_date: i64,
    pub end_date: i64,
    pub frequency: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ModifyOrderRequest {
    pub order_id: OrderId,
    pub quantity_change: f64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CancelOrderRequest {
    pub order_id: OrderId,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InitResponse {
    pub backtest_id: BacktestId,
    pub depth: DateDepth,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DatasetInfoResponse {
    pub start_date: i64,
    pub end_date: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InfoResponse {
    pub version: String,
    pub dataset: String,
}

#[derive(Debug)]
pub enum UistV2Error {
    UnknownBacktest,
    UnknownDataset,
}

impl std::error::Error for UistV2Error {}

impl core::fmt::Display for UistV2Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            UistV2Error::UnknownBacktest => write!(f, "UnknownBacktest"),
            UistV2Error::UnknownDataset => write!(f, "UnknownDataset"),
        }
    }
}

impl actix_web::ResponseError for UistV2Error {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            UistV2Error::UnknownBacktest => actix_web::http::StatusCode::BAD_REQUEST,
            UistV2Error::UnknownDataset => actix_web::http::StatusCode::BAD_REQUEST,
        }
    }
}


pub trait Client {
    fn tick(&self, backtest_id: BacktestId) -> impl Future<Output = Result<TickResponse>>;
    fn insert_orders(
        &self,
        orders: Vec<Order>,
        backtest_id: BacktestId,
    ) -> impl Future<Output = Result<()>>;
    fn init(
        &self,
        dataset_name: String,
        start_date: i64,
        end_date: i64,
        frequency: u64,
    ) -> impl Future<Output = Result<InitResponse>>;
    fn info(&self, backtest_id: BacktestId) -> impl Future<Output = Result<InfoResponse>>;
    fn dataset_info(
        &self,
        dataset_name: String,
    ) -> impl Future<Output = Result<DatasetInfoResponse>>;
}

type UistState = AppState;

pub mod server {
    use actix_web::{get, post, web};

    use super::{
        BacktestId, DatasetInfoResponse, InfoResponse, InitRequest, InitResponse,
        InsertOrderRequest, TickResponse, UistState, UistV2Error,
    };

    #[get("/backtest/{backtest_id}/tick")]
    pub async fn tick(
        app: web::Data<UistState>,
        path: web::Path<(BacktestId,)>,
    ) -> Result<web::Json<TickResponse>, UistV2Error> {
        let (backtest_id,) = path.into_inner();

        if let Some(result) = app.tick(backtest_id).await {
            Ok(web::Json(TickResponse {
                depth: result.3,
                inserted_orders: result.2,
                executed_orders: result.1,
                has_next: result.0,
                now: result.4,
                taker_trades: result.5,
            }))
        } else {
            Err(UistV2Error::UnknownBacktest)
        }
    }

    #[post("/backtest/{backtest_id}/insert_orders")]
    pub async fn insert_orders(
        app: web::Data<UistState>,
        path: web::Path<(BacktestId,)>,
        mut insert_order: web::Json<InsertOrderRequest>,
    ) -> Result<web::Json<()>, UistV2Error> {
        let (backtest_id,) = path.into_inner();
        let take_orders = std::mem::take(&mut insert_order.orders);
        if let Some(()) = app.insert_orders(take_orders, backtest_id) {
            Ok(web::Json(()))
        } else {
            Err(UistV2Error::UnknownBacktest)
        }
    }

    #[post("/init/{dataset_name}")]
    pub async fn init(
        app: web::Data<UistState>,
        path: web::Path<(String,)>,
        init: web::Json<InitRequest>,
    ) -> Result<web::Json<InitResponse>, UistV2Error> {
        let (dataset_name,) = path.into_inner();

        if let Some((backtest_id, depth)) =
            app.init(dataset_name, init.start_date, init.end_date, init.frequency).await
        {
            Ok(web::Json(InitResponse {
                backtest_id,
                depth,
            }))
        } else {
            Err(UistV2Error::UnknownDataset)
        }
    }

    #[get("/backtest/{backtest_id}/info")]
    pub async fn info(
        app: web::Data<UistState>,
        path: web::Path<(BacktestId,)>,
    ) -> Result<web::Json<InfoResponse>, UistV2Error> {
        let (backtest_id,) = path.into_inner();

        if let Some(resp) = app.backtests.get(&backtest_id) {
            Ok(web::Json(InfoResponse {
                version: "v1".to_string(),
                dataset: resp.dataset_name.clone(),
            }))
        } else {
            Err(UistV2Error::UnknownBacktest)
        }
    }

    #[get("/dataset/{dataset_name}/info")]
    pub async fn dataset_info(
        app: web::Data<UistState>,
        path: web::Path<(String,)>,
    ) -> Result<web::Json<DatasetInfoResponse>, UistV2Error> {
        let (dataset_name,) = path.into_inner();

        if let Some(resp) = app.dataset_info(&dataset_name).await {
            Ok(web::Json(DatasetInfoResponse {
                start_date: resp.0,
                end_date: resp.1,
            }))
        } else {
            Err(UistV2Error::UnknownDataset)
        }
    }
}
