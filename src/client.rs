use std::time::Duration;

use base64::Engine;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use url::Url;

use crate::error::Fail;

pub(crate) const DEFAULT_AUTHORIZE: &str = "https://boosty.to/app/oauth2/authorize";
pub(crate) const DEFAULT_TOKEN: &str = "https://api.boosty.to/oauth/server/token";
pub(crate) const DEFAULT_DEVAPI: &str = "https://devapi.boosty.to/v1";
pub(crate) const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub(crate) struct Hosts {
    pub authorize: Url,
    pub token: Url,
    pub devapi: Url,
}

pub(crate) fn resolve_hosts(
    authorize: Option<&str>,
    token: Option<&str>,
    devapi: Option<&str>,
) -> Result<Hosts, Fail> {
    Ok(Hosts {
        authorize: parse_base(authorize.unwrap_or(DEFAULT_AUTHORIZE))?,
        token: parse_base(token.unwrap_or(DEFAULT_TOKEN))?,
        devapi: parse_base(devapi.unwrap_or(DEFAULT_DEVAPI))?,
    })
}

fn parse_base(value: &str) -> Result<Url, Fail> {
    Url::parse(value).map_err(|_| Fail::bad_base_url())
}

pub(crate) fn basic_token(client_id: &str, secret: &str) -> String {
    let raw = format!("{client_id}:{secret}");
    base64::engine::general_purpose::STANDARD.encode(raw.as_bytes())
}

pub(crate) fn user_agent() -> String {
    format!("boosty-cli/{}", env!("CARGO_PKG_VERSION"))
}

#[derive(Deserialize)]
pub(crate) struct TokenResponse {
    pub access_token: String,
    pub expires_in: i64,
    pub token_type: String,
    pub refresh_token: String,
}

impl std::fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenResponse")
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .field("token_type", &self.token_type)
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

pub(crate) struct ApiClient {
    http: reqwest::Client,
    token: Url,
    devapi: Url,
}

impl ApiClient {
    pub(crate) fn build(timeout: Duration, hosts: Hosts) -> Result<Self, Fail> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .user_agent(user_agent())
            .default_headers(headers)
            .build()
            .map_err(|_| Fail::bad_base_url())?;
        Ok(Self {
            http,
            token: hosts.token,
            devapi: hosts.devapi,
        })
    }

    pub(crate) async fn post_token<T: Serialize + ?Sized>(
        &self,
        client_id: &str,
        secret: &str,
        form: &T,
    ) -> Result<TokenResponse, Fail> {
        let header = format!("Basic {}", basic_token(client_id, secret));
        let response = self
            .http
            .post(self.token.clone())
            .header(reqwest::header::AUTHORIZATION, header)
            .form(form)
            .send()
            .await
            .map_err(|_| Fail::token_transport())?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|_| Fail::token_transport())?;
        if status.is_success() {
            serde_json::from_slice(&bytes).map_err(|_| Fail::token_body())
        } else if status.is_client_error() {
            Err(Fail::token_rejected())
        } else {
            Err(Fail::token_transport())
        }
    }

    pub(crate) async fn get_user<T: DeserializeOwned>(
        &self,
        access_token: &str,
    ) -> Result<T, Fail> {
        let url = self.devapi_url("user")?;
        let authorization = format!("Bearer {access_token}");
        let response = self
            .http
            .get(url)
            .header(reqwest::header::AUTHORIZATION, authorization)
            .send()
            .await
            .map_err(|_| Fail::user_api())?;
        let status = response.status();
        let bytes = response.bytes().await.map_err(|_| Fail::user_api())?;
        if !status.is_success() {
            return Err(Fail::user_api());
        }
        serde_json::from_slice(&bytes).map_err(|_| Fail::user_api())
    }

    pub(crate) async fn subscription_levels<T: DeserializeOwned>(
        &self,
        api_key: &str,
    ) -> Result<T, Fail> {
        self.get_with_api_key("subscription_levels", api_key).await
    }

    pub(crate) async fn subscriptions<T: DeserializeOwned>(
        &self,
        api_key: &str,
    ) -> Result<T, Fail> {
        self.get_with_api_key("subscriptions", api_key).await
    }

    pub(crate) async fn subscribers<T: DeserializeOwned>(
        &self,
        api_key: &str,
        is_free: Option<bool>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<T, Fail> {
        let mut url = self.devapi_url("subscribers")?;
        let pairs = subscriber_query(is_free, limit, offset);
        // Empty pairs must not touch the query serializer. A bare '?' makes the
        // recorded query non-empty and breaks the no-query DevAPI reads.
        if !pairs.is_empty() {
            let mut query = url.query_pairs_mut();
            for (key, value) in &pairs {
                query.append_pair(key, value);
            }
        }
        self.send_api_key(url, api_key).await
    }

    async fn get_with_api_key<T: DeserializeOwned>(
        &self,
        segment: &str,
        api_key: &str,
    ) -> Result<T, Fail> {
        let url = self.devapi_url(segment)?;
        self.send_api_key(url, api_key).await
    }

    async fn send_api_key<T: DeserializeOwned>(&self, url: Url, api_key: &str) -> Result<T, Fail> {
        let response = self
            .http
            .get(url)
            .header("X-Api-Key", api_key)
            .send()
            .await
            .map_err(|_| Fail::devapi())?;
        let status = response.status();
        let bytes = response.bytes().await.map_err(|_| Fail::devapi())?;
        if !status.is_success() {
            return Err(Fail::devapi());
        }
        serde_json::from_slice(&bytes).map_err(|_| Fail::devapi())
    }

    fn devapi_url(&self, segment: &str) -> Result<Url, Fail> {
        let mut base = self.devapi.clone();
        // Url::join replaces the last segment unless the path ends with '/'.
        if !base.path().ends_with('/') {
            base.path_segments_mut()
                .map_err(|_| Fail::bad_base_url())?
                .push("");
        }
        base.join(segment).map_err(|_| Fail::bad_base_url())
    }
}

fn subscriber_query(
    is_free: Option<bool>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Vec<(&'static str, String)> {
    let mut pairs = Vec::new();
    if let Some(value) = is_free {
        pairs.push((
            "is_free",
            if value {
                "true".to_string()
            } else {
                "false".to_string()
            },
        ));
    }
    if let Some(value) = limit {
        pairs.push(("limit", value.to_string()));
    }
    if let Some(value) = offset {
        pairs.push(("offset", value.to_string()));
    }
    pairs
}
