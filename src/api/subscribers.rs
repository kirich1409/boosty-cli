use serde::Deserialize;
use serde::Serialize;

use crate::client::ApiClient;
use crate::error::Fail;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[allow(non_snake_case)]
pub(crate) struct SubscriberFlags {
    pub isAuthor: bool,
    pub isBanned: bool,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[allow(non_snake_case)]
pub(crate) struct SubscriberUser {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub avatarUrl: String,
    pub flags: SubscriberFlags,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[allow(non_snake_case)]
pub(crate) struct SubscriberSubscription {
    pub id: i64,
    pub ownerId: i64,
    pub levelId: i64,
    pub startTime: i64,
    pub endTime: i64,
    pub nextPayTime: i64,
    pub subscriberId: i64,
    pub payments: String,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[allow(non_snake_case)]
pub(crate) struct SubscribersExtra {
    pub nextOffset: i64,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SubscribersResponse {
    pub users: Vec<SubscriberUser>,
    pub subscriptions: Vec<SubscriberSubscription>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_extra"
    )]
    pub extra: Option<SubscribersExtra>,
}

#[derive(Deserialize)]
struct RawExtra {
    #[serde(
        default,
        rename = "nextOffset",
        deserialize_with = "deserialize_present_i64"
    )]
    next_offset: Option<i64>,
}

fn deserialize_extra<'de, D>(deserializer: D) -> Result<Option<SubscribersExtra>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<RawExtra>::deserialize(deserializer)?;
    Ok(raw.and_then(|raw| {
        raw.next_offset.map(|next_offset| SubscribersExtra {
            nextOffset: next_offset,
        })
    }))
}

fn deserialize_present_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(number) => {
            let Some(offset) = number.as_i64() else {
                return Err(serde::de::Error::custom("nextOffset"));
            };
            Ok(Some(offset))
        }
        _ => Err(serde::de::Error::custom("nextOffset")),
    }
}

impl std::fmt::Debug for SubscriberFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscriberFlags")
            .field("isAuthor", &self.isAuthor)
            .field("isBanned", &self.isBanned)
            .finish()
    }
}

impl std::fmt::Debug for SubscriberUser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscriberUser")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("avatarUrl", &self.avatarUrl)
            .field("flags", &self.flags)
            .finish()
    }
}

impl std::fmt::Debug for SubscriberSubscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscriberSubscription")
            .field("id", &self.id)
            .field("ownerId", &self.ownerId)
            .field("levelId", &self.levelId)
            .field("startTime", &self.startTime)
            .field("endTime", &self.endTime)
            .field("nextPayTime", &self.nextPayTime)
            .field("subscriberId", &self.subscriberId)
            .field("payments", &self.payments)
            .finish()
    }
}

impl std::fmt::Debug for SubscribersExtra {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscribersExtra")
            .field("nextOffset", &self.nextOffset)
            .finish()
    }
}

impl std::fmt::Debug for SubscribersResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscribersResponse")
            .field("users", &self.users)
            .field("subscriptions", &self.subscriptions)
            .field("extra", &self.extra)
            .finish()
    }
}

pub(crate) async fn fetch(
    http: &ApiClient,
    api_key: &str,
    is_free: Option<bool>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<SubscribersResponse, Fail> {
    http.subscribers(api_key, is_free, limit, offset).await
}
