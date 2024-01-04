use reqwest::Result;

use crate::exchange::uist::{InitMessage, UistOrder, UistOrderId};
use crate::server::uist::{
    DeleteOrderRequest, FetchQuotesResponse, InsertOrderRequest, TickResponse,
};

pub struct UistClient {
    pub path: String,
}

impl UistClient {
    pub async fn tick(&self) -> Result<TickResponse> {
        reqwest::get(self.path.clone() + "/tick")
            .await?
            .json::<TickResponse>()
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
