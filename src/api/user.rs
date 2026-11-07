use serde::{Deserialize, Serialize};

use crate::client::ApiClient;
use crate::error::Fail;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[allow(non_snake_case)]
pub(crate) struct UserFlags {
    pub isAuthor: bool,
    pub isBanned: bool,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[allow(non_snake_case)]
pub(crate) struct User {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub avatarUrl: String,
    pub flags: UserFlags,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct UserResponse {
    pub user: User,
}

impl std::fmt::Debug for UserFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserFlags")
            .field("isAuthor", &self.isAuthor)
            .field("isBanned", &self.isBanned)
            .finish()
    }
}

impl std::fmt::Debug for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("User")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("avatarUrl", &self.avatarUrl)
            .field("flags", &self.flags)
            .finish()
    }
}

impl std::fmt::Debug for UserResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserResponse")
            .field("user", &self.user)
            .finish()
    }
}

pub(crate) async fn fetch(http: &ApiClient, access_token: &str) -> Result<UserResponse, Fail> {
    http.get_user(access_token).await
}
