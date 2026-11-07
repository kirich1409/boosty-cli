use serde::{Deserialize, Serialize};

use crate::client::ApiClient;
use crate::error::Fail;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[allow(non_snake_case)]
pub(crate) struct SubscriptionLevel {
    pub id: i64,
    pub price: String,
    pub authorId: i64,
    pub name: String,
    pub createTime: i64,
    pub isDeleted: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[allow(non_snake_case)]
pub(crate) struct Subscription {
    pub id: i64,
    pub ownerId: i64,
    pub levelId: i64,
    pub startTime: i64,
    pub endTime: i64,
    pub nextPayTime: i64,
    pub subscriberId: i64,
    pub payments: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[allow(non_snake_case)]
pub(crate) struct SubscriptionLevelsResponse {
    pub subscriptionLevels: Vec<SubscriptionLevel>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[allow(non_snake_case)]
pub(crate) struct SubscriptionsResponse {
    pub subscriptions: Vec<Subscription>,
    pub subscriptionLevels: Vec<SubscriptionLevel>,
}

pub(crate) async fn fetch_subscription_levels(
    http: &ApiClient,
    api_key: &str,
) -> Result<SubscriptionLevelsResponse, Fail> {
    http.subscription_levels(api_key).await
}

pub(crate) async fn fetch_subscriptions(
    http: &ApiClient,
    api_key: &str,
) -> Result<SubscriptionsResponse, Fail> {
    http.subscriptions(api_key).await
}
