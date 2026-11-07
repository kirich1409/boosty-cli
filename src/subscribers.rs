use crate::api::subscribers::{self, SubscribersResponse};
use crate::client::ApiClient;
use crate::error::Fail;

pub(crate) async fn run_subscribers(
    api_key: Option<&str>,
    http: &ApiClient,
    is_free: Option<bool>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<SubscribersResponse, Fail> {
    let Some(api_key) = api_key.filter(|value| !value.is_empty()) else {
        return Err(Fail::missing_api_key());
    };
    subscribers::fetch(http, api_key, is_free, limit, offset).await
}
