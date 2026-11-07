use std::io;
use std::path::Path;
use std::time::SystemTime;

use crate::api;
use crate::auth;
use crate::client::ApiClient;
use crate::config::{self, ReadTokens};
use crate::error::Fail;

const REFRESH_WITHIN_SECS: i64 = 60;

pub(crate) async fn run_user<R, C>(
    client_id: Option<&str>,
    client_secret: Option<&str>,
    path: &Path,
    http: &ApiClient,
    clock: C,
    rename: R,
) -> Result<api::user::UserResponse, Fail>
where
    R: FnOnce(&Path, &Path) -> io::Result<()>,
    C: Fn() -> SystemTime,
{
    let stored = match config::read_tokens(path) {
        ReadTokens::Missing => return Err(Fail::missing_token_file()),
        ReadTokens::Invalid | ReadTokens::MissingRefresh => return Err(Fail::token_file()),
        ReadTokens::Ready(tokens) => tokens,
    };
    let now = config::unix_seconds(clock())?;
    let horizon = now
        .checked_add(REFRESH_WITHIN_SECS)
        .ok_or_else(Fail::expiry_overflow)?;
    let stored = if stored.expires_at <= horizon {
        // run_refresh persists the rotated tokens and does not return them.
        auth::run_refresh(client_id, client_secret, path, http, clock, rename).await?;
        match config::read_tokens(path) {
            ReadTokens::Ready(tokens) => tokens,
            _ => return Err(Fail::token_file()),
        }
    } else {
        stored
    };
    if stored.token_type != "Bearer" {
        return Err(Fail::not_bearer());
    }
    api::user::fetch(http, &stored.access_token).await
}
