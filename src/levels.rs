use crate::api::levels::{self, SubscriptionLevelsResponse, SubscriptionsResponse};
use crate::client::ApiClient;
use crate::error::Fail;

pub(crate) async fn run_subscription_levels(
    api_key: Option<&str>,
    http: &ApiClient,
) -> Result<SubscriptionLevelsResponse, Fail> {
    let Some(api_key) = api_key else {
        return Err(Fail::missing_api_key());
    };
    levels::fetch_subscription_levels(http, api_key).await
}

pub(crate) async fn run_subscriptions(
    api_key: Option<&str>,
    http: &ApiClient,
) -> Result<SubscriptionsResponse, Fail> {
    let Some(api_key) = api_key else {
        return Err(Fail::missing_api_key());
    };
    levels::fetch_subscriptions(http, api_key).await
}
