use std::io::{self, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use url::Url;

use crate::client::{ApiClient, TokenResponse};
use crate::config::{self, StoredTokens};
use crate::error::Fail;
use crate::output::SessionView;

pub(crate) const LOGIN_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BindTarget {
    pub host: String,
    pub port: u16,
}

pub(crate) trait Binder {
    fn bind(&mut self, target: &BindTarget) -> io::Result<Vec<TcpListener>>;
}

pub(crate) struct SocketBinder;

impl Binder for SocketBinder {
    fn bind(&mut self, target: &BindTarget) -> io::Result<Vec<TcpListener>> {
        use std::net::ToSocketAddrs;
        let host = if target.host.contains(':') {
            format!("[{}]", target.host)
        } else {
            target.host.clone()
        };
        let addrs: Vec<_> = format!("{host}:{}", target.port)
            .to_socket_addrs()?
            .collect();
        if addrs.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                "no addresses",
            ));
        }
        let mut listeners = Vec::new();
        for addr in addrs {
            if addr.port() != target.port {
                return Err(io::Error::new(
                    io::ErrorKind::AddrNotAvailable,
                    "port changed",
                ));
            }
            listeners.push(TcpListener::bind(addr)?);
        }
        Ok(listeners)
    }
}

pub(crate) struct LoginRequest {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub redirect_url: String,
    pub authorize_endpoint: Url,
    pub http: ApiClient,
    pub token_path: PathBuf,
    pub timeout: Duration,
}

pub(crate) struct LoginDeps<O, B, R, C> {
    pub opener: O,
    pub binder: B,
    pub rename: R,
    pub clock: C,
}

pub(crate) fn new_state() -> Result<String, Fail> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| Fail::state_entropy())?;
    let mut hex = String::with_capacity(64);
    for byte in bytes {
        hex.push_str(&format!("{byte:02x}"));
    }
    Ok(hex)
}

pub(crate) fn authorize_url(
    endpoint: &Url,
    client_id: &str,
    redirect_url: &str,
    state: &str,
) -> Result<Url, Fail> {
    let mut url = endpoint.clone();
    url.set_query(None);
    url.set_fragment(None);
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("client_id", client_id);
        query.append_pair("redirect_url", redirect_url);
        query.append_pair("response_type", "code");
        query.append_pair("state", state);
    }
    Ok(url)
}

pub(crate) fn redirect_target(raw: &str) -> Result<(Url, BindTarget), Fail> {
    let url = Url::parse(raw).map_err(|_| Fail::bad_redirect())?;
    if url.scheme() != "http" {
        return Err(Fail::bad_redirect());
    }
    let port = url.port().ok_or_else(Fail::bad_redirect)?;
    let host = loopback_host(&url).ok_or_else(Fail::bad_redirect)?;
    Ok((url, BindTarget { host, port }))
}

fn loopback_host(url: &Url) -> Option<String> {
    match url.host()? {
        url::Host::Domain(name) if name.eq_ignore_ascii_case("localhost") => Some(name.to_string()),
        url::Host::Ipv4(ip) if ip.is_loopback() => Some(ip.to_string()),
        url::Host::Ipv6(ip) if ip.is_loopback() => Some(ip.to_string()),
        _ => None,
    }
}

pub(crate) fn credentials<'a>(
    client_id: Option<&'a str>,
    client_secret: Option<&'a str>,
) -> Result<(&'a str, &'a str), Fail> {
    match (client_id, client_secret) {
        (Some(id), Some(secret)) if !id.is_empty() && !secret.is_empty() => Ok((id, secret)),
        _ => Err(Fail::missing_credentials()),
    }
}

pub(crate) async fn run_login<O, B, R, C, W>(
    request: LoginRequest,
    mut deps: LoginDeps<O, B, R, C>,
    stderr: &mut W,
) -> Result<SessionView, Fail>
where
    O: FnOnce(&str) -> io::Result<()> + Send + 'static,
    B: Binder,
    R: FnOnce(&Path, &Path) -> io::Result<()>,
    C: Fn() -> SystemTime,
    W: Write,
{
    let (redirect, target) = redirect_target(&request.redirect_url)?;
    let (client_id, secret) = credentials(
        request.client_id.as_deref(),
        request.client_secret.as_deref(),
    )?;
    let listeners = deps.binder.bind(&target).map_err(|_| Fail::bind_failed())?;
    let state = new_state()?;
    let authorize = authorize_url(
        &request.authorize_endpoint,
        client_id,
        &request.redirect_url,
        &state,
    )?;
    let authorize_text = authorize.to_string();
    let opener = deps.opener;
    let opened = tokio::task::spawn_blocking(move || opener(&authorize_text)).await;
    if !matches!(opened, Ok(Ok(()))) {
        let _ = writeln!(stderr, "{authorize}");
        let _ = stderr.flush();
    }

    let callback = wait_for_callback(listeners, request.timeout, redirect.path(), &state).await?;
    // redirect_url on authorize and redirect_uri in this form are an intentional name split
    // for the same registered URL text.
    let response = exchange_code(
        &request.http,
        client_id,
        secret,
        &request.redirect_url,
        &callback,
    )
    .await?;
    save_response(&request.token_path, response, &deps.clock, deps.rename)
}

#[derive(Serialize)]
struct CodeExchange<'a> {
    grant_type: &'a str,
    redirect_uri: &'a str,
    code: &'a str,
}

#[derive(Serialize)]
struct RefreshGrant<'a> {
    grant_type: &'a str,
    refresh_token: &'a str,
}

async fn exchange_code(
    http: &ApiClient,
    client_id: &str,
    secret: &str,
    redirect_url: &str,
    code: &str,
) -> Result<TokenResponse, Fail> {
    http.post_token(
        client_id,
        secret,
        &CodeExchange {
            grant_type: "authorization_code",
            redirect_uri: redirect_url,
            code,
        },
    )
    .await
}

pub(crate) async fn run_refresh<R, C>(
    client_id: Option<&str>,
    client_secret: Option<&str>,
    path: &Path,
    http: &ApiClient,
    clock: C,
    rename: R,
) -> Result<SessionView, Fail>
where
    R: FnOnce(&Path, &Path) -> io::Result<()>,
    C: Fn() -> SystemTime,
{
    let stored = match config::read_tokens(path) {
        config::ReadTokens::Missing => return Err(Fail::missing_token_file()),
        config::ReadTokens::MissingRefresh => return Err(Fail::missing_refresh_token()),
        config::ReadTokens::Invalid => return Err(Fail::token_file()),
        config::ReadTokens::Ready(tokens) => tokens,
    };
    let (client_id, secret) = credentials(client_id, client_secret)?;
    let response = http
        .post_token(
            client_id,
            secret,
            &RefreshGrant {
                grant_type: "refresh_token",
                refresh_token: &stored.refresh_token,
            },
        )
        .await?;
    save_response(path, response, &clock, rename)
}

pub(crate) fn run_status(path: &Path) -> Result<SessionView, Fail> {
    match config::read_tokens(path) {
        config::ReadTokens::Missing => Err(Fail::missing_token_file()),
        config::ReadTokens::Ready(tokens) => Ok(SessionView {
            expires_at: tokens.expires_at,
            path: path.to_path_buf(),
        }),
        config::ReadTokens::MissingRefresh | config::ReadTokens::Invalid => Err(Fail::token_file()),
    }
}

fn save_response(
    path: &Path,
    response: TokenResponse,
    clock: &impl Fn() -> SystemTime,
    rename: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> Result<SessionView, Fail> {
    let tokens = StoredTokens {
        access_token: response.access_token,
        refresh_token: response.refresh_token,
        token_type: response.token_type,
        expires_at: config::expires_at(clock(), response.expires_in)?,
    };
    config::persist_tokens(path, &tokens, rename)?;
    match config::read_tokens(path) {
        config::ReadTokens::Ready(stored) => Ok(SessionView {
            expires_at: stored.expires_at,
            path: path.to_path_buf(),
        }),
        _ => Err(Fail::token_file()),
    }
}

struct AbortOnDrop(Vec<tokio::task::JoinHandle<()>>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        for handle in &self.0 {
            handle.abort();
        }
    }
}

async fn wait_for_callback(
    listeners: Vec<TcpListener>,
    timeout: Duration,
    expected_path: &str,
    state: &str,
) -> Result<String, Fail> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(4);
    let mut tasks = Vec::new();
    for listener in listeners {
        listener
            .set_nonblocking(true)
            .map_err(|_| Fail::bind_failed())?;
        let listener =
            tokio::net::TcpListener::from_std(listener).map_err(|_| Fail::bind_failed())?;
        let tx = tx.clone();
        tasks.push(tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                if tx.send(stream).await.is_err() {
                    break;
                }
            }
        }));
    }
    drop(tx);
    let _abort = AbortOnDrop(tasks);

    let wait = async {
        loop {
            let Some(mut stream) = rx.recv().await else {
                return Err(Fail::bind_failed());
            };
            let Some(line) = read_request_line(&mut stream).await else {
                let _ = write_browser(&mut stream, "Некорректный запрос.").await;
                continue;
            };
            match classify_callback(&line, expected_path, state) {
                Callback::Ignore => {
                    let _ = write_browser(&mut stream, "Не найдено.").await;
                }
                Callback::Fail(fail) => {
                    let _ = write_browser(&mut stream, "Авторизация отклонена.").await;
                    return Err(fail);
                }
                Callback::Code(code) => {
                    let _ = write_browser(&mut stream, "Авторизация получена. Можно закрыть окно.")
                        .await;
                    return Ok(code);
                }
            }
        }
    };
    match tokio::time::timeout(timeout, wait).await {
        Ok(result) => result,
        Err(_) => Err(Fail::login_timeout()),
    }
}

async fn read_request_line(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while buf.len() < 8192 {
        if let Some(end) = buf.windows(2).position(|w| w == b"\r\n") {
            return std::str::from_utf8(&buf[..end]).ok().map(str::to_string);
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        match tokio::time::timeout(remaining, stream.read(&mut tmp)).await {
            Ok(Ok(n)) if n > 0 => buf.extend_from_slice(&tmp[..n]),
            _ => return None,
        }
    }
    None
}

async fn write_browser(stream: &mut TcpStream, body: &str) -> io::Result<()> {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await?;
    let _ = stream.shutdown().await;
    Ok(())
}

enum Callback {
    Ignore,
    Fail(Fail),
    Code(String),
}

fn classify_callback(line: &str, expected_path: &str, state: &str) -> Callback {
    let mut parts = line.split_whitespace();
    let Some(method) = parts.next() else {
        return Callback::Ignore;
    };
    let Some(target) = parts.next() else {
        return Callback::Ignore;
    };
    if !target.starts_with('/') {
        return Callback::Ignore;
    }
    let parsed = Url::parse(&format!("http://localhost{target}")).ok();
    let path = parsed
        .as_ref()
        .map(|url| url.path().to_string())
        .unwrap_or_else(|| target.split('?').next().unwrap_or(target).to_string());
    if path != expected_path {
        return Callback::Ignore;
    }
    let Some(url) = parsed else {
        return Callback::Fail(Fail::bad_callback());
    };
    if method != "GET" {
        return Callback::Fail(Fail::bad_callback());
    }
    let query: Vec<(String, String)> = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    if query
        .iter()
        .any(|(key, _)| key == "error" || key == "error_reason")
    {
        return Callback::Fail(Fail::denied());
    }
    match query
        .iter()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.as_str())
    {
        Some(got) if got == state => {}
        _ => return Callback::Fail(Fail::state_mismatch()),
    }
    match query
        .iter()
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value.as_str())
    {
        Some(code) if !code.is_empty() => Callback::Code(code.to_string()),
        _ => Callback::Fail(Fail::missing_code()),
    }
}
