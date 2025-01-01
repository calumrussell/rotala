use std::collections::BTreeMap;
use std::future::Future;
use std::sync::atomic::AtomicU64;

use anyhow::Result;
use dashmap::try_result::TryResult;
use dashmap::DashMap;
use deadpool_postgres::Pool;
use rotala::input::minerva::Minerva;
use serde::{Deserialize, Serialize};

use rotala::exchange::uist_v2::{InnerOrder, Order, OrderId, OrderResult, UistV2};
use rotala::source::hyperliquid::{DateDepth, Trade};

pub type BacktestId = u64;
pub type TickResponseType = (
    bool,
    Vec<OrderResult>,
    Vec<InnerOrder>,
    DateDepth,
    i64,
    Vec<Trade>,
);

pub struct BacktestState {
    pub id: BacktestId,
    pub start_date: i64,
    pub curr_date: i64,
    pub frequency: u64,
    pub end_date: i64,
    pub exchange: UistV2,
}

pub struct AppState {
    pub backtests: DashMap<BacktestId, BacktestState>,
    pub datasets: DashMap<BacktestId, Minerva>,
    pub pool: Pool,
    pub last: AtomicU64,
}

impl AppState {
    const MAX_BACKTEST_LENGTH: i64 = 1_000_000;

    pub fn create_db_pool(user: &str, dbname: &str, host: &str, password: &str) -> Pool {
        let mut pg_config = tokio_postgres::Config::new();
        pg_config.user(user);
        pg_config.dbname(dbname);
        pg_config.host(host);
        pg_config.password(password);

        let mgr_config = deadpool_postgres::ManagerConfig {
            recycling_method: deadpool_postgres::RecyclingMethod::Fast,
        };
        let mgr =
            deadpool_postgres::Manager::from_config(pg_config, tokio_postgres::NoTls, mgr_config);
        Pool::builder(mgr).max_size(16).build().unwrap()
    }

    pub fn create(user: &str, dbname: &str, host: &str, password: &str) -> Self {
        Self {
            backtests: DashMap::new(),
            last: AtomicU64::new(0),
            pool: Self::create_db_pool(user, dbname, host, password),
            datasets: DashMap::new(),
        }
    }

    pub fn single(user: &str, dbname: &str, host: &str, password: &str) -> Self {
        let minerva = Minerva::new();

        let datasets = DashMap::new();
        datasets.insert(0, minerva);

        Self {
            backtests: DashMap::new(),
            last: AtomicU64::new(1),
            pool: Self::create_db_pool(user, dbname, host, password),
            datasets,
        }
    }

    pub async fn tick(&self, backtest_id: BacktestId) -> Option<TickResponseType> {
        if let TryResult::Present(mut backtest) = self.backtests.try_get_mut(&backtest_id) {
            if let Some(dataset) = self.datasets.get(&backtest_id) {
                let mut executed_orders = Vec::new();
                let mut inserted_orders = Vec::new();

                let curr_date = backtest.curr_date;

                let back_depth = dataset
                    .get_depth_between(
                        backtest.curr_date - backtest.frequency as i64..backtest.curr_date,
                    )
                    .await;
                if let Some((_date, back_depth_last)) = back_depth.last() {
                    let back_trades = dataset
                        .get_trades_between(
                            backtest.curr_date - backtest.frequency as i64..backtest.curr_date,
                        )
                        .await;
                    let mut back_trades_last = BTreeMap::default();
                    if let Some((date, back_trades_last_query)) = back_trades.last() {
                        back_trades_last.insert(*date, back_trades_last_query.to_vec());
                    }
                    let mut res =
                        backtest
                            .exchange
                            .tick(back_depth_last, &back_trades_last, curr_date);

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
                        Vec::new(),
                    ));
                } else {
                    let depth = dataset
                        .get_depth_between(backtest.curr_date..new_date)
                        .await;
                    let mut last_depth = BTreeMap::default();
                    if let Some((_date, queried_last_quotes)) = depth.last() {
                        //TODO: not great, as state is stored in DB it isn't clear why we need
                        //clone
                        last_depth = queried_last_quotes.clone();
                    }
                    let trades = dataset
                        .get_trades_between(backtest.curr_date..new_date)
                        .await;
                    let mut last_trades = Vec::default();
                    if let Some((_date, queried_last_trades)) = trades.last() {
                        //TODO: not great either
                        for trade in queried_last_trades {
                            last_trades.push(trade.clone());
                        }
                    }

                    backtest.curr_date = new_date;
                    return Some((
                        true,
                        executed_orders,
                        inserted_orders,
                        last_depth,
                        new_date,
                        last_trades,
                    ));
                }
            }
        }
        None
    }

    pub async fn init(
        &self,
        start_date: i64,
        end_date: i64,
        frequency: u64,
    ) -> Option<(BacktestId, DateDepth)> {
        let curr_id = self.last.load(std::sync::atomic::Ordering::SeqCst);
        let exchange = UistV2::new();

        let mut minerva = Minerva::new();
        minerva.init_cache(&self.pool, start_date..end_date).await;

        let dataset_date_bounds = minerva.get_date_bounds(&self.pool).await.unwrap();
        let mut end_date_backtest = if dataset_date_bounds.1 > end_date {
            end_date
        } else {
            dataset_date_bounds.1
        };

        let backtest_length = (end_date_backtest - start_date) / frequency as i64;
        if backtest_length > Self::MAX_BACKTEST_LENGTH {
            end_date_backtest = start_date + (frequency as i64 * Self::MAX_BACKTEST_LENGTH);
        }

        let backtest = BacktestState {
            id: curr_id,
            start_date,
            curr_date: start_date,
            end_date: end_date_backtest,
            frequency,
            exchange,
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

                let depth = minerva
                    .get_depth_between(start_date - frequency as i64..start_date)
                    .await;
                let mut last_depth = BTreeMap::default();
                if let Some((_date, last_value)) = depth.last() {
                    //TODO: isn't clear why clone is required here, same as above somewhere
                    last_depth = last_value.clone();
                }

                self.datasets.insert(curr_id, minerva);
                return Some((curr_id, last_depth));
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

    pub async fn dataset_info(&self) -> Option<(i64, i64)> {
        let minerva = Minerva::new();
        return minerva.get_date_bounds(&self.pool).await;
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TickResponse {
    pub has_next: bool,
    pub executed_orders: Vec<OrderResult>,
    pub inserted_orders: Vec<InnerOrder>,
    pub depth: DateDepth,
    pub taker_trades: Vec<Trade>,
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
        start_date: i64,
        end_date: i64,
        frequency: u64,
    ) -> impl Future<Output = Result<InitResponse>>;
    fn info(&self, backtest_id: BacktestId) -> impl Future<Output = Result<InfoResponse>>;
    fn dataset_info(&self) -> impl Future<Output = Result<DatasetInfoResponse>>;
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

    #[post("/init")]
    pub async fn init(
        app: web::Data<UistState>,
        _path: web::Path<()>,
        init: web::Json<InitRequest>,
    ) -> Result<web::Json<InitResponse>, UistV2Error> {
        if let Some((backtest_id, depth)) = app
            .init(init.start_date, init.end_date, init.frequency)
            .await
        {
            Ok(web::Json(InitResponse { backtest_id, depth }))
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

        if let Some(_resp) = app.backtests.get(&backtest_id) {
            Ok(web::Json(InfoResponse {
                version: "v1".to_string(),
            }))
        } else {
            Err(UistV2Error::UnknownBacktest)
        }
    }

    #[get("/dataset/info")]
    pub async fn dataset_info(
        app: web::Data<UistState>,
    ) -> Result<web::Json<DatasetInfoResponse>, UistV2Error> {
        if let Some(resp) = app.dataset_info().await {
            Ok(web::Json(DatasetInfoResponse {
                start_date: resp.0,
                end_date: resp.1,
            }))
        } else {
            Err(UistV2Error::UnknownDataset)
        }
    }
}
