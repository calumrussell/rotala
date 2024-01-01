use reqwest::Result;

use crate::api::uist::{
    CheckResponse, DeleteOrderRequest, FetchQuotesResponse, FetchTradesRequest,
    FetchTradesResponse, InsertOrderRequest,
};
use crate::exchange::uist::{InitMessage, UistOrder, UistOrderId};

pub struct UistClient {
    pub path: String,
}

impl UistClient {
    pub async fn check(&self) -> Result<CheckResponse> {
        reqwest::get(self.path.clone() + "/check")
            .await?
            .json::<CheckResponse>()
            .await
    }

    pub async fn delete_order(&self, order_id: UistOrderId) -> Result<()> {
        let req = DeleteOrderRequest { order_id };
        let client = reqwest::Client::new();
        client
            .post(self.path.clone() + "/delete_order")
            .json(&req)
            .send()
            .await?
            .json::<()>()
            .await
    }

    pub async fn insert_order(&self, order: UistOrder) -> Result<()> {
        let req = InsertOrderRequest { order };
        let client = reqwest::Client::new();
        client
            .post(self.path.clone() + "/insert_order")
            .json(&req)
            .send()
            .await?
            .json::<()>()
            .await
    }

    pub async fn fetch_trades(&self, from: usize) -> Result<FetchTradesResponse> {
        let req = FetchTradesRequest { from };
        let client = reqwest::Client::new();
        client
            .post(self.path.clone() + "/fetch_tradse")
            .json(&req)
            .send()
            .await?
            .json::<FetchTradesResponse>()
            .await
    }

    pub async fn fetch_quotes(&self) -> Result<FetchQuotesResponse> {
        reqwest::get(self.path.clone() + "/fetch_quotes")
            .await?
            .json::<FetchQuotesResponse>()
            .await
    }

    pub async fn init(&self) -> Result<InitMessage> {
        reqwest::get(self.path.clone() + "/init")
            .await?
            .json::<InitMessage>()
            .await
    }

    pub fn new(path: String) -> Self {
        Self { path }
    }
}
