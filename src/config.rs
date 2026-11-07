use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::Fail;

pub(crate) struct Env {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub redirect_url: Option<String>,
    pub token_path: Option<String>,
    pub xdg_config_home: Option<String>,
    pub home: Option<String>,
    pub authorize_url: Option<String>,
    pub token_url: Option<String>,
    pub devapi_base: Option<String>,
}

impl Env {
    pub(crate) fn from_process() -> Self {
        Self {
            client_id: non_empty("BOOSTY_CLIENT_ID"),
            client_secret: non_empty("BOOSTY_CLIENT_SECRET"),
            redirect_url: non_empty("BOOSTY_REDIRECT_URL"),
            token_path: non_empty("BOOSTY_TOKEN_PATH"),
            xdg_config_home: non_empty("XDG_CONFIG_HOME"),
            home: non_empty("HOME"),
            authorize_url: non_empty("BOOSTY_AUTHORIZE_URL"),
            token_url: non_empty("BOOSTY_TOKEN_URL"),
            devapi_base: non_empty("BOOSTY_DEVAPI_BASE"),
        }
    }
}

fn non_empty(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

pub(crate) fn token_path(
    override_path: Option<&str>,
    xdg_config_home: Option<&str>,
    home: Option<&str>,
) -> Result<PathBuf, Fail> {
    if let Some(path) = override_path.filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    if let Some(xdg) = xdg_config_home.filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(xdg).join("boosty-cli").join("tokens.json"));
    }
    if let Some(home) = home.filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(home)
            .join(".config")
            .join("boosty-cli")
            .join("tokens.json"));
    }
    Err(Fail::no_home())
}

#[derive(Serialize, Deserialize)]
pub(crate) struct StoredTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_at: i64,
}

impl std::fmt::Debug for StoredTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredTokens")
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .field("token_type", &self.token_type)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

pub(crate) enum ReadTokens {
    Missing,
    MissingRefresh,
    Invalid,
    Ready(StoredTokens),
}

pub(crate) fn read_tokens(path: &Path) -> ReadTokens {
    if !path.exists() {
        return ReadTokens::Missing;
    }
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => return ReadTokens::Invalid,
    };
    let value: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(_) => return ReadTokens::Invalid,
    };
    let refresh_absent = value
        .as_object()
        .is_none_or(|object| !object.contains_key("refresh_token"));
    if !value.is_object() {
        return ReadTokens::Invalid;
    }
    match serde_json::from_value::<StoredTokens>(value) {
        Ok(tokens) => ReadTokens::Ready(tokens),
        Err(_) if refresh_absent => ReadTokens::MissingRefresh,
        Err(_) => ReadTokens::Invalid,
    }
}

pub(crate) fn unix_seconds(now: SystemTime) -> Result<i64, Fail> {
    let now_secs = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Fail::expiry_overflow())?;
    i64::try_from(now_secs.as_secs()).map_err(|_| Fail::expiry_overflow())
}

pub(crate) fn expires_at(now: SystemTime, expires_in: i64) -> Result<i64, Fail> {
    unix_seconds(now)?
        .checked_add(expires_in)
        .ok_or_else(Fail::expiry_overflow)
}

pub(crate) fn persist_tokens(
    path: &Path,
    tokens: &StoredTokens,
    rename: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> Result<(), Fail> {
    let bytes = serde_json::to_vec(tokens).map_err(|_| Fail::token_file())?;
    let parent = parent_dir(path);
    if let Some(dir) = parent {
        ensure_private_dir(dir)?;
    }
    let tmp = temp_path(path);
    write_temp(&tmp, &bytes)?;
    if let Err(error) = rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        let _ = error;
        return Err(Fail::token_file());
    }
    #[cfg(unix)]
    set_mode(path, 0o600)?;
    Ok(())
}

fn parent_dir(path: &Path) -> Option<&Path> {
    path.parent().filter(|dir| !dir.as_os_str().is_empty())
}

fn ensure_private_dir(dir: &Path) -> Result<(), Fail> {
    if dir.exists() {
        return Ok(());
    }
    if let Some(parent) = parent_dir(dir) {
        ensure_private_dir(parent)?;
    }
    match fs::create_dir(dir) {
        Ok(()) => {
            #[cfg(unix)]
            set_mode(dir, 0o700)?;
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(_) => Err(Fail::token_file()),
    }
}

fn write_temp(path: &Path, bytes: &[u8]) -> Result<(), Fail> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|_| Fail::token_file())?;
    file.write_all(bytes).map_err(|_| Fail::token_file())?;
    file.sync_all().map_err(|_| Fail::token_file())?;
    #[cfg(unix)]
    set_mode(path, 0o600)?;
    Ok(())
}

fn temp_path(final_path: &Path) -> PathBuf {
    let dir = parent_dir(final_path).unwrap_or_else(|| Path::new("."));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    dir.join(format!(".tokens-{}-{nanos}.tmp", std::process::id()))
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), Fail> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|_| Fail::token_file())
}
