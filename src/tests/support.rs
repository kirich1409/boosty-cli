use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::oneshot;
use url::Url;

use crate::auth::{BindTarget, Binder, LoginDeps, LoginRequest};
use crate::client::{self, ApiClient};
use crate::config::StoredTokens;
use crate::error::Fail;
use crate::output::SessionView;

pub(crate) const ID: &str = "client-id-test";
pub(crate) const SECRET: &str = "secret-test-value";
pub(crate) const CODE: &str = "code-test-value";
pub(crate) const CLOCK: u64 = 1_700_000_000;

pub(crate) struct Tmp {
    path: PathBuf,
}

impl Tmp {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "boosty-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Hit {
    pub method: String,
    pub path: String,
    pub query: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

pub(crate) enum Reply {
    Status { status: u16, body: String },
    Silent,
    Close,
}

pub(crate) struct Server {
    addr: SocketAddr,
    hits: Arc<Mutex<Vec<Hit>>>,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    pub(crate) fn spawn(reply: Reply) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let hits_thread = Arc::clone(&hits);
        let shutdown_thread = Arc::clone(&shutdown);
        let thread = thread::spawn(move || serve(listener, reply, hits_thread, shutdown_thread));
        Self {
            addr,
            hits,
            shutdown,
            thread: Some(thread),
        }
    }

    pub(crate) fn token_url(&self) -> String {
        format!("http://{}/oauth/server/token", self.addr)
    }

    pub(crate) fn base(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub(crate) fn hits(&self) -> Vec<Hit> {
        self.hits.lock().unwrap().clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(
    listener: TcpListener,
    reply: Reply,
    hits: Arc<Mutex<Vec<Hit>>>,
    shutdown: Arc<AtomicBool>,
) {
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                if let Some(hit) = read_hit(&mut stream) {
                    hits.lock().unwrap().push(hit);
                }
                match &reply {
                    Reply::Status { status, body } => {
                        let reason = match *status {
                            200..=299 => "OK",
                            400..=499 => "Bad Request",
                            _ => "Error",
                        };
                        let header = format!(
                            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(header.as_bytes());
                        let _ = stream.write_all(body.as_bytes());
                    }
                    Reply::Silent => {
                        while !shutdown.load(Ordering::Relaxed) {
                            thread::sleep(Duration::from_millis(20));
                        }
                    }
                    Reply::Close => drop(stream),
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }
}

fn read_hit(stream: &mut TcpStream) -> Option<Hit> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 2048];
    loop {
        if request_complete(&buf) {
            break;
        }
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(_) => break,
        }
    }
    parse_hit(&buf)
}

fn request_complete(buf: &[u8]) -> bool {
    let Some(end) = find_subsequence(buf, b"\r\n\r\n") else {
        return false;
    };
    let headers = String::from_utf8_lossy(&buf[..end]);
    let len = content_length(&headers).unwrap_or(0);
    buf.len() >= end + 4 + len
}

fn content_length(headers: &str) -> Option<usize> {
    headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("content-length") {
            value.trim().parse().ok()
        } else {
            None
        }
    })
}

fn parse_hit(buf: &[u8]) -> Option<Hit> {
    let text = String::from_utf8_lossy(buf);
    let (head, _) = text.split_once("\r\n").unwrap_or((text.as_ref(), ""));
    let mut parts = head.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let (path, query) = target
        .split_once('?')
        .map(|(path, query)| (path.to_string(), query.to_string()))
        .unwrap_or((target, String::new()));
    let mut headers = Vec::new();
    for line in text.split("\r\n").skip(1) {
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
        }
    }
    let body_start = find_subsequence(buf, b"\r\n\r\n")
        .map(|i| i + 4)
        .unwrap_or(buf.len());
    let body = String::from_utf8_lossy(&buf[body_start..]).into_owned();
    Some(Hit {
        method,
        path,
        query,
        headers,
        body,
    })
}

fn find_subsequence(buf: &[u8], needle: &[u8]) -> Option<usize> {
    buf.windows(needle.len())
        .position(|window| window == needle)
}

pub(crate) fn header<'a>(hit: &'a Hit, name: &str) -> Vec<&'a str> {
    hit.headers
        .iter()
        .filter(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
        .collect()
}

pub(crate) fn form_pairs(body: &str) -> Vec<(String, String)> {
    let url = Url::parse(&format!("http://localhost/?{body}")).unwrap();
    url.query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

pub(crate) fn token_body(access: &str, refresh: &str, token_type: &str, expires_in: i64) -> String {
    serde_json::json!({
        "access_token": access,
        "expires_in": expires_in,
        "token_type": token_type,
        "refresh_token": refresh,
        "ignored_field": "ignored-test-1"
    })
    .to_string()
}

pub(crate) fn client_for(token_url: &str, timeout: Duration) -> ApiClient {
    let hosts = client::resolve_hosts(
        Some("http://127.0.0.1:9/authorize"),
        Some(token_url),
        Some("http://127.0.0.1:9/v1"),
    )
    .unwrap();
    ApiClient::build(timeout, hosts).unwrap()
}

pub(crate) fn fixed_clock(secs: u64) -> impl Fn() -> SystemTime {
    move || UNIX_EPOCH + Duration::from_secs(secs)
}

pub(crate) fn write_stored(path: &Path, tokens: &StoredTokens) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, serde_json::to_vec(tokens).unwrap()).unwrap();
}

pub(crate) fn sample_tokens(refresh: &str) -> StoredTokens {
    StoredTokens {
        access_token: "access-test-1".to_string(),
        refresh_token: refresh.to_string(),
        token_type: "TestType".to_string(),
        expires_at: CLOCK as i64,
    }
}

pub(crate) struct CaptureBinder {
    pub calls: Arc<Mutex<Vec<BindTarget>>>,
}

impl Binder for CaptureBinder {
    fn bind(&mut self, target: &BindTarget) -> std::io::Result<Vec<TcpListener>> {
        self.calls.lock().unwrap().push(target.clone());
        Err(std::io::Error::other("bind stopped"))
    }
}

pub(crate) struct RecordBinder<B> {
    pub inner: B,
    pub calls: Arc<Mutex<Vec<BindTarget>>>,
}

impl<B: Binder> Binder for RecordBinder<B> {
    fn bind(&mut self, target: &BindTarget) -> std::io::Result<Vec<TcpListener>> {
        self.calls.lock().unwrap().push(target.clone());
        self.inner.bind(target)
    }
}

pub(crate) fn calls_of() -> Arc<Mutex<Vec<BindTarget>>> {
    Arc::new(Mutex::new(Vec::new()))
}

pub(crate) async fn run_with(
    request: LoginRequest,
    opener: impl FnOnce(&str) -> std::io::Result<()> + Send + 'static,
    binder: impl Binder + Send + 'static,
    rename: impl FnOnce(&Path, &Path) -> std::io::Result<()> + Send + 'static,
    clock: impl Fn() -> SystemTime + Send + 'static,
    stderr: &mut Vec<u8>,
) -> Result<SessionView, Fail> {
    crate::auth::run_login(
        request,
        LoginDeps {
            opener,
            binder,
            rename,
            clock,
        },
        stderr,
    )
    .await
}

pub(crate) struct Started {
    pub task: tokio::task::JoinHandle<(Result<SessionView, Fail>, Vec<u8>)>,
    pub opened: oneshot::Receiver<String>,
}

pub(crate) fn start_login(
    request: LoginRequest,
    browser_ok: bool,
    binder: impl Binder + Send + 'static,
    rename: impl FnOnce(&Path, &Path) -> std::io::Result<()> + Send + 'static,
    clock_secs: u64,
) -> Started {
    let (tx, opened) = oneshot::channel();
    let opener = move |url: &str| {
        let _ = tx.send(url.to_string());
        if browser_ok {
            Ok(())
        } else {
            Err(std::io::Error::other("browser failed"))
        }
    };
    let task = tokio::spawn(async move {
        let mut stderr = Vec::new();
        let result = run_with(
            request,
            opener,
            binder,
            rename,
            fixed_clock(clock_secs),
            &mut stderr,
        )
        .await;
        (result, stderr)
    });
    Started { task, opened }
}

pub(crate) fn login_request(
    token_url: &str,
    redirect: &str,
    path: PathBuf,
    timeout: Duration,
    id: Option<&str>,
    secret: Option<&str>,
) -> LoginRequest {
    LoginRequest {
        client_id: id.map(str::to_string),
        client_secret: secret.map(str::to_string),
        redirect_url: redirect.to_string(),
        authorize_endpoint: Url::parse("http://127.0.0.1:9/authorize").unwrap(),
        http: client_for(token_url, Duration::from_secs(5)),
        token_path: path,
        timeout,
    }
}

pub(crate) struct HeldBinder {
    listener: Option<TcpListener>,
}

impl Binder for HeldBinder {
    fn bind(&mut self, target: &BindTarget) -> std::io::Result<Vec<TcpListener>> {
        let listener = self
            .listener
            .take()
            .ok_or_else(|| std::io::Error::other("listener already taken"))?;
        let addr = listener.local_addr()?;
        if target.host != "127.0.0.1" || target.port != addr.port() {
            return Err(std::io::Error::other("bind target mismatch"));
        }
        Ok(vec![listener])
    }
}

pub(crate) fn hold_port() -> (u16, HeldBinder) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    (
        port,
        HeldBinder {
            listener: Some(listener),
        },
    )
}

pub(crate) fn send_callback(addr: &str, target: &str) -> String {
    let mut last = String::new();
    for _ in 0..50 {
        match TcpStream::connect(addr) {
            Ok(mut stream) => {
                let req =
                    format!("GET {target} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
                stream.write_all(req.as_bytes()).unwrap();
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut buf = Vec::new();
                let _ = stream.read_to_end(&mut buf);
                return String::from_utf8_lossy(&buf).into_owned();
            }
            Err(error) => {
                last = error.to_string();
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
    panic!("callback connect failed: {last}");
}

pub(crate) fn assert_kind(err: &Fail, kind: crate::error::Kind) {
    assert_eq!(err.kind(), kind);
    assert_eq!(err.code(), kind.code());
}

pub(crate) fn boosty_bin() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_boosty") {
        return PathBuf::from(path);
    }
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("boosty")
}

pub(crate) fn assert_basic(hit: &Hit) {
    let auths = header(hit, "authorization");
    assert_eq!(auths.len(), 1, "{:?}", hit.headers);
    assert!(auths[0].starts_with("Basic "), "{}", auths[0]);
    let encoded = auths[0].trim_start_matches("Basic ");
    let decoded =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded).unwrap();
    assert_eq!(decoded, format!("{ID}:{SECRET}").as_bytes());
    assert!(hit.headers.iter().all(|(key, value)| {
        key != "x-api-key" && key != "authorization" || value.starts_with("Basic ")
    }));
    assert!(
        hit.headers
            .iter()
            .all(|(key, value)| key != "authorization" || !value.starts_with("Bearer"))
    );
    assert!(hit.headers.iter().all(|(key, _)| key != "x-api-key"));
}

pub(crate) fn assert_token_request(hit: &Hit) {
    assert_eq!(hit.method, "POST");
    assert_eq!(hit.path, "/oauth/server/token");
    assert!(hit.query.is_empty(), "{}", hit.query);
    assert_eq!(header(hit, "accept"), vec!["application/json"]);
    assert_eq!(
        header(hit, "user-agent"),
        vec![client::user_agent().as_str()]
    );
    assert_basic(hit);
    assert!(!hit.body.contains(SECRET), "{}", hit.body);
}

pub(crate) fn rename_ok(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::rename(from, to)
}

pub(crate) fn state_of(url: &str) -> String {
    Url::parse(url)
        .unwrap()
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned()
}

pub(crate) fn query_map(url: &str) -> Vec<(String, String)> {
    Url::parse(url)
        .unwrap()
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}
