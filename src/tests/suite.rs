use std::path::Path;
use std::time::{Duration, Instant};

use base64::Engine;

use super::support::*;
use crate::auth::{self, new_state, redirect_target};
use crate::client::{self, TokenResponse};
use crate::config::{self, StoredTokens};
use crate::error::Fail;
use crate::error::Kind;
use crate::output::{self, SessionView};

fn fixture_text() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/token_response.json"),
    )
    .unwrap()
}

fn contract_text() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("authorization_code_flow.md"),
    )
    .unwrap()
}

#[test]
fn token_fixture_decodes_and_ignores_unknown_field() {
    let parsed: TokenResponse = serde_json::from_str(&fixture_text()).unwrap();
    assert_eq!(parsed.access_token, "access-test-1");
    assert_eq!(parsed.expires_in, 100);
    assert_eq!(parsed.token_type, "TestType");
    assert_eq!(parsed.refresh_token, "refresh-test-1");
    let again = serde_json::json!({
        "access_token": parsed.access_token,
        "expires_in": parsed.expires_in,
        "token_type": parsed.token_type,
        "refresh_token": parsed.refresh_token,
    });
    assert!(again.get("ignored_field").is_none());
}

#[test]
fn token_fixture_missing_each_required_field_is_an_error() {
    let value: serde_json::Value = serde_json::from_str(&fixture_text()).unwrap();
    for key in ["access_token", "expires_in", "token_type", "refresh_token"] {
        let mut broken = value.clone();
        broken.as_object_mut().unwrap().remove(key);
        assert!(
            serde_json::from_value::<TokenResponse>(broken).is_err(),
            "{key}"
        );
    }
}

#[test]
fn fixture_does_not_copy_contract_samples() {
    let fixture = fixture_text();
    let contract = contract_text();
    let parsed: serde_json::Value = serde_json::from_str(&fixture).unwrap();
    let mut fixture_strings = Vec::new();
    collect_strings(&parsed, &mut fixture_strings);
    for value in &fixture_strings {
        assert!(!contract.contains(value), "{value}");
    }
    for sample in json_strings(&contract) {
        assert!(!fixture.contains(&sample), "{sample}");
    }
}

fn collect_strings(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => out.push(text.clone()),
        serde_json::Value::Array(items) => {
            for item in items {
                collect_strings(item, out);
            }
        }
        serde_json::Value::Object(map) => {
            for item in map.values() {
                collect_strings(item, out);
            }
        }
        _ => {}
    }
}

fn json_strings(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'{'
            && let Some(end) = matching_brace(text, index)
        {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text[index..=end]) {
                collect_strings(&value, &mut out);
            }
            index = end + 1;
            continue;
        }
        index += 1;
    }
    out
}

fn matching_brace(text: &str, start: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for (offset, ch) in text[start..].char_indices() {
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + offset);
                }
            }
            _ => {}
        }
    }
    None
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expires_at_is_injected_clock_plus_expires_in() {
    let tmp = Tmp::new();
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-test-1", "refresh-test-1", "TestType", 100),
    });
    let path = tmp.child("tokens.json");
    let result = consent_login(&server, path.clone(), None, true).await;
    assert!(result.result.is_ok(), "{:?}", result.result);
    let stored: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(stored["expires_at"], CLOCK as i64 + 100);
    assert!(stored.get("expires_in").is_none());
    assert!(stored.get("secret").is_none());
    assert!(stored.get("api_key").is_none());
    let text = String::from_utf8(std::fs::read(&path).unwrap()).unwrap();
    assert!(!text.contains(SECRET));
    assert!(!text.contains("BOOSTY_API_KEY"));
}

#[test]
fn basic_is_standard_base64_with_padding() {
    let token = client::basic_token("id+", "sec/ret");
    let raw = b"id+:sec/ret";
    assert_eq!(token, base64::engine::general_purpose::STANDARD.encode(raw));
    assert!(token.ends_with('='));
    assert_ne!(
        token,
        base64::engine::general_purpose::STANDARD_NO_PAD.encode(raw)
    );
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&token)
        .unwrap();
    assert_eq!(decoded, raw);
    assert!(!contract_text().contains(&token));
}

#[test]
fn default_hosts_live_in_client() {
    let hosts = client::resolve_hosts(None, None, None).unwrap();
    assert_eq!(hosts.authorize.as_str(), client::DEFAULT_AUTHORIZE);
    assert_eq!(hosts.token.as_str(), client::DEFAULT_TOKEN);
    assert_eq!(hosts.devapi.as_str(), client::DEFAULT_DEVAPI);
    let client_src = read_src("client.rs");
    for literal in [
        client::DEFAULT_AUTHORIZE,
        client::DEFAULT_TOKEN,
        client::DEFAULT_DEVAPI,
    ] {
        assert!(client_src.contains(literal), "{literal}");
        for file in ["cli.rs", "auth.rs", "main.rs", "config.rs", "output.rs"] {
            assert!(!read_src(file).contains(literal), "{file} {literal}");
        }
    }
}

#[test]
fn url_overrides_replace_whole_url() {
    let hosts = client::resolve_hosts(
        Some("http://127.0.0.1:11/auth"),
        Some("http://127.0.0.1:12/token"),
        Some("http://127.0.0.1:13/v1"),
    )
    .unwrap();
    assert_eq!(hosts.authorize.as_str(), "http://127.0.0.1:11/auth");
    assert_eq!(hosts.token.as_str(), "http://127.0.0.1:12/token");
    assert_eq!(hosts.devapi.as_str(), "http://127.0.0.1:13/v1");
    let err = client::resolve_hosts(Some("not a url"), None, None).unwrap_err();
    assert_kind(&err, Kind::Io);
    let proc = run_bin(
        &["auth", "status"],
        &[
            ("BOOSTY_DEVAPI_BASE", "not a url"),
            ("HOME", "/tmp"),
            ("BOOSTY_TOKEN_PATH", "/tmp/unused-tokens.json"),
        ],
    );
    assert_eq!(proc.code, 5);
    assert!(proc.stdout.is_empty());
}

#[test]
fn manifest_uses_rustls_without_native_tls_or_blocking() {
    let text =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    let reqwest = text.lines().find(|line| line.contains("reqwest")).unwrap();
    assert!(reqwest.contains("default-features = false"), "{reqwest}");
    assert!(reqwest.contains("rustls"), "{reqwest}");
    assert!(!reqwest.contains("native-tls"), "{reqwest}");
    assert!(!reqwest.contains("blocking"), "{reqwest}");
    assert!(!text.to_ascii_lowercase().contains("openssl"));
}

#[test]
fn http_timeout_constant_is_thirty_seconds() {
    assert_eq!(client::HTTP_TIMEOUT, Duration::from_secs(30));
    assert!(read_src("main.rs").contains("HTTP_TIMEOUT"));
}

#[test]
fn login_timeout_constant_is_three_minutes() {
    assert_eq!(auth::LOGIN_TIMEOUT, Duration::from_secs(180));
    assert!(read_src("main.rs").contains("LOGIN_TIMEOUT"));
}

#[test]
fn token_call_site_comments_redirect_name_split() {
    let src = read_src("auth.rs");
    let comment = src
        .lines()
        .find(|line| {
            line.trim_start().starts_with("//")
                && line.contains("redirect_url")
                && line.contains("redirect_uri")
        })
        .expect("comment");
    assert!(comment.contains("redirect_url"));
    assert!(comment.contains("redirect_uri"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_timeout_is_api_failure() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    let prior = b"prior-bytes";
    std::fs::write(&path, prior).unwrap();
    let server = Server::spawn(Reply::Silent);
    let http = client_for(&server.token_url(), Duration::from_millis(200));
    let started = Instant::now();
    let err = http
        .post_token(
            ID,
            SECRET,
            &serde_json::json!({"grant_type": "refresh_token", "refresh_token": "refresh-test-1"}),
        )
        .await
        .unwrap_err();
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_kind(&err, Kind::Api);
    assert_eq!(std::fs::read(&path).unwrap(), prior);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exchange_posts_exact_authorization_code_form() {
    let tmp = Tmp::new();
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-test-1", "refresh-new", "TestType", 100),
    });
    let done = consent_login(&server, tmp.child("tokens.json"), None, true).await;
    assert!(done.result.is_ok(), "{:?}", done.result);
    assert_eq!(server.hits().len(), 1);
    let hit = &server.hits()[0];
    assert_token_request(hit);
    let pairs = form_pairs(&hit.body);
    assert_eq!(pairs.len(), 3, "{}", hit.body);
    assert!(
        pairs
            .iter()
            .any(|(k, v)| k == "grant_type" && v == "authorization_code")
    );
    assert!(
        pairs
            .iter()
            .any(|(k, v)| k == "redirect_uri" && v == &done.redirect)
    );
    assert!(pairs.iter().any(|(k, v)| k == "code" && v == CODE));
    assert!(!pairs.iter().any(|(k, _)| k == "redirect_url"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refresh_posts_exact_refresh_form() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_stored(&path, &sample_tokens("refresh-old"));
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-new", "refresh-new", "TestType", 100),
    });
    let http = client_for(&server.token_url(), Duration::from_secs(5));
    let view = auth::run_refresh(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        rename_ok,
    )
    .await
    .unwrap();
    assert_eq!(server.hits().len(), 1);
    let hit = &server.hits()[0];
    assert_token_request(hit);
    let pairs = form_pairs(&hit.body);
    assert_eq!(pairs.len(), 2, "{}", hit.body);
    assert!(
        pairs
            .iter()
            .any(|(k, v)| k == "grant_type" && v == "refresh_token")
    );
    assert!(
        pairs
            .iter()
            .any(|(k, v)| k == "refresh_token" && v == "refresh-old")
    );
    assert!(
        !pairs
            .iter()
            .any(|(k, _)| k == "redirect_uri" || k == "code")
    );
    let stored: StoredTokens = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(stored.refresh_token, "refresh-new");
    assert_eq!(view.expires_at, stored.expires_at);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authorize_query_uses_redirect_url_not_redirect_uri() {
    let tmp = Tmp::new();
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-test-1", "refresh-test-1", "TestType", 100),
    });
    let done = consent_login(&server, tmp.child("tokens.json"), Some("x=1&y=2"), true).await;
    assert!(done.result.is_ok(), "{:?}", done.result);
    let pairs = query_map(&done.authorize);
    let names: Vec<_> = pairs.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        names,
        vec!["client_id", "redirect_url", "response_type", "state"]
    );
    assert!(!names.contains(&"redirect_uri"));
    assert!(!done.authorize.contains(SECRET));
    assert_eq!(
        pairs
            .iter()
            .find(|(key, _)| key == "response_type")
            .unwrap()
            .1,
        "code"
    );
    assert_eq!(
        pairs
            .iter()
            .find(|(key, _)| key == "redirect_url")
            .unwrap()
            .1,
        done.redirect
    );
    assert_eq!(
        pairs.iter().find(|(key, _)| key == "client_id").unwrap().1,
        ID
    );
    let form = form_pairs(&server.hits()[0].body);
    assert_eq!(
        form.iter()
            .find(|(key, _)| key == "redirect_uri")
            .unwrap()
            .1,
        done.redirect
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_login_attempts_get_different_cryptographic_states() {
    let first = capture_state().await;
    let second = capture_state().await;
    assert_ne!(first, second);
    assert!(is_state(&first), "{first}");
    assert!(is_state(&second), "{second}");
    let direct_a = new_state().unwrap();
    let direct_b = new_state().unwrap();
    assert_ne!(direct_a, direct_b);
    assert!(is_state(&direct_a));
}

fn is_state(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn read_src(name: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(name)).unwrap()
}

struct Done {
    result: Result<SessionView, Fail>,
    stderr: Vec<u8>,
    authorize: String,
    redirect: String,
}

async fn consent_login(
    server: &Server,
    path: std::path::PathBuf,
    extra_query: Option<&str>,
    browser_ok: bool,
) -> Done {
    let (port, binder) = hold_port();
    let redirect = match extra_query {
        Some(query) => format!("http://127.0.0.1:{port}/cb?{query}"),
        None => format!("http://127.0.0.1:{port}/cb"),
    };
    let started = start_login(
        login_request(
            &server.token_url(),
            &redirect,
            path,
            Duration::from_secs(5),
            Some(ID),
            Some(SECRET),
        ),
        browser_ok,
        binder,
        rename_ok,
        CLOCK,
    );
    let authorize = tokio::time::timeout(Duration::from_secs(2), started.opened)
        .await
        .unwrap()
        .unwrap();
    let _browser = send_callback(
        &format!("127.0.0.1:{port}"),
        &format!("/cb?state={}&code={CODE}", state_of(&authorize)),
    );
    let (result, stderr) = tokio::time::timeout(Duration::from_secs(5), started.task)
        .await
        .unwrap()
        .unwrap();
    Done {
        result,
        stderr,
        authorize,
        redirect,
    }
}

async fn capture_state() -> String {
    let tmp = Tmp::new();
    let (port, binder) = hold_port();
    let redirect = format!("http://127.0.0.1:{port}/cb");
    let started = start_login(
        login_request(
            "http://127.0.0.1:9/oauth/server/token",
            &redirect,
            tmp.child("tokens.json"),
            Duration::from_millis(200),
            Some(ID),
            Some(SECRET),
        ),
        true,
        binder,
        rename_ok,
        CLOCK,
    );
    let authorize = tokio::time::timeout(Duration::from_secs(2), started.opened)
        .await
        .unwrap()
        .unwrap();
    started.task.abort();
    let _ = started.task.await;
    state_of(&authorize)
}

struct Proc {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run_bin(args: &[&str], env: &[(&str, &str)]) -> Proc {
    let mut cmd = std::process::Command::new(boosty_bin());
    cmd.args(args).env_clear();
    for (key, value) in env {
        cmd.env(key, value);
    }
    let out = cmd.output().unwrap();
    Proc {
        code: out.status.code().unwrap(),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_client_id_or_secret_is_auth_and_does_not_bind() {
    for (id, secret) in [
        (None, Some(SECRET)),
        (Some(ID), None),
        (Some(""), Some(SECRET)),
        (Some(ID), Some("")),
    ] {
        let calls = calls_of();
        let tmp = Tmp::new();
        let server = Server::spawn(Reply::Status {
            status: 200,
            body: token_body("access-test-1", "refresh-test-1", "TestType", 100),
        });
        let mut stderr = Vec::new();
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let err = run_with(
            login_request(
                &server.token_url(),
                "http://127.0.0.1:9/cb",
                tmp.child("tokens.json"),
                Duration::from_millis(50),
                id,
                secret,
            ),
            move |url: &str| {
                let _ = tx.send(url.to_string());
                Ok(())
            },
            CaptureBinder {
                calls: std::sync::Arc::clone(&calls),
            },
            rename_ok,
            fixed_clock(CLOCK),
            &mut stderr,
        )
        .await
        .unwrap_err();
        assert_kind(&err, Kind::Auth);
        assert!(calls.lock().unwrap().is_empty());
        assert!(rx.try_recv().is_err());
        assert!(server.hits().is_empty());
    }
    let tmp = Tmp::new();
    let server = Server::spawn(Reply::Close);
    let proc = run_bin(
        &["--json", "auth", "login"],
        &[
            ("HOME", tmp.path().to_str().unwrap()),
            ("BOOSTY_REDIRECT_URL", "http://127.0.0.1:9/cb"),
            ("BOOSTY_TOKEN_URL", &server.token_url()),
        ],
    );
    assert_eq!(proc.code, 3);
    assert!(proc.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_str(proc.stderr.trim()).unwrap();
    assert_eq!(error["kind"], "auth");
    assert!(server.hits().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bad_redirect_url_is_io_before_bind() {
    let cases = [
        "not a url",
        "http://192.0.2.1:9/cb",
        "http://127.0.0.1/cb",
        "https://127.0.0.1:9/cb",
    ];
    for raw in cases {
        let calls = calls_of();
        let tmp = Tmp::new();
        let server = Server::spawn(Reply::Close);
        let mut stderr = Vec::new();
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let err = run_with(
            login_request(
                &server.token_url(),
                raw,
                tmp.child("tokens.json"),
                Duration::from_millis(50),
                Some(ID),
                Some(SECRET),
            ),
            move |url: &str| {
                let _ = tx.send(url.to_string());
                Ok(())
            },
            CaptureBinder {
                calls: std::sync::Arc::clone(&calls),
            },
            rename_ok,
            fixed_clock(CLOCK),
            &mut stderr,
        )
        .await
        .unwrap_err();
        assert_kind(&err, Kind::Io);
        assert!(calls.lock().unwrap().is_empty(), "{raw}");
        assert!(rx.try_recv().is_err(), "{raw}");
        assert!(server.hits().is_empty(), "{raw}");
    }
    let tmp = Tmp::new();
    let server = Server::spawn(Reply::Close);
    let proc = run_bin(
        &["auth", "login"],
        &[
            ("HOME", tmp.path().to_str().unwrap()),
            ("BOOSTY_CLIENT_ID", ID),
            ("BOOSTY_CLIENT_SECRET", SECRET),
            ("BOOSTY_REDIRECT_URL", "http://192.0.2.1:9/cb"),
            ("BOOSTY_TOKEN_URL", &server.token_url()),
        ],
    );
    assert_eq!(proc.code, 5);
    assert!(proc.stdout.is_empty());
    assert!(server.hits().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bind_uses_host_and_port_from_redirect_url() {
    let localhost = recorded_bind("http://localhost:9/cb").await;
    assert_eq!(localhost.host, "localhost");
    assert_eq!(localhost.port, 9);
    let v4 = recorded_bind("http://127.0.0.1:9/cb").await;
    assert_eq!(v4.host, "127.0.0.1");
    assert_eq!(v4.port, 9);
    let v6 = recorded_bind("http://[::1]:9/cb").await;
    assert_eq!(v6.host, "::1");
    assert_eq!(v6.port, 9);

    let hold = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = hold.local_addr().unwrap().port();
    let calls = calls_of();
    let tmp = Tmp::new();
    let mut started = start_login(
        login_request(
            "http://127.0.0.1:9/oauth/server/token",
            &format!("http://127.0.0.1:{port}/cb"),
            tmp.child("tokens.json"),
            Duration::from_millis(200),
            Some(ID),
            Some(SECRET),
        ),
        true,
        RecordBinder {
            inner: crate::auth::SocketBinder,
            calls: std::sync::Arc::clone(&calls),
        },
        rename_ok,
        CLOCK,
    );
    let (result, _) = tokio::time::timeout(Duration::from_secs(2), started.task)
        .await
        .unwrap()
        .unwrap();
    assert_kind(&result.unwrap_err(), Kind::Io);
    assert!(started.opened.try_recv().is_err());
    let recorded = calls.lock().unwrap().clone();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].port, port);
    assert_eq!(recorded[0].host, "127.0.0.1");
}

async fn recorded_bind(url: &str) -> crate::auth::BindTarget {
    let calls = calls_of();
    let tmp = Tmp::new();
    let mut stderr = Vec::new();
    let err = run_with(
        login_request(
            "http://127.0.0.1:9/oauth/server/token",
            url,
            tmp.child("tokens.json"),
            Duration::from_millis(50),
            Some(ID),
            Some(SECRET),
        ),
        |_| Ok(()),
        CaptureBinder {
            calls: std::sync::Arc::clone(&calls),
        },
        rename_ok,
        fixed_clock(CLOCK),
        &mut stderr,
    )
    .await
    .unwrap_err();
    assert_kind(&err, Kind::Io);
    let recorded = calls.lock().unwrap().clone();
    assert!(!recorded.is_empty(), "{url} {:?}", err.english());
    recorded[0].clone()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_loopback_classifier_rejects_before_bind() {
    let calls = calls_of();
    let tmp = Tmp::new();
    let mut stderr = Vec::new();
    let err = run_with(
        login_request(
            "http://127.0.0.1:9/oauth/server/token",
            "http://192.0.2.1:9/cb",
            tmp.child("tokens.json"),
            Duration::from_millis(50),
            Some(ID),
            Some(SECRET),
        ),
        |_| Ok(()),
        CaptureBinder {
            calls: std::sync::Arc::clone(&calls),
        },
        rename_ok,
        fixed_clock(CLOCK),
        &mut stderr,
    )
    .await
    .unwrap_err();
    assert_kind(&err, Kind::Io);
    assert!(calls.lock().unwrap().is_empty());
    assert!(redirect_target("http://192.0.2.1:9/cb").is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browser_failure_prints_authorize_url_on_stderr_and_login_can_still_succeed() {
    let tmp = Tmp::new();
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-test-1", "refresh-test-1", "TestType", 100),
    });
    let done = consent_login(&server, tmp.child("tokens.json"), None, false).await;
    assert!(done.result.is_ok(), "{:?}", done.result);
    let stderr = String::from_utf8(done.stderr).unwrap();
    assert!(stderr.contains(&done.authorize), "{stderr}");
    let stdout = output::render_success(false, &done.result.unwrap());
    assert!(!stdout.contains(&done.authorize));
    assert_eq!(server.hits().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browser_success_does_not_print_authorize_url() {
    let tmp = Tmp::new();
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-test-1", "refresh-test-1", "TestType", 100),
    });
    let done = consent_login(&server, tmp.child("tokens.json"), None, true).await;
    assert!(done.result.is_ok(), "{:?}", done.result);
    let stderr = String::from_utf8(done.stderr.clone()).unwrap();
    assert!(stderr.is_empty(), "{stderr}");
    let stdout = output::render_success(false, done.result.as_ref().unwrap());
    assert!(!stdout.contains(&done.authorize));
    assert!(!stderr.contains(&done.authorize));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wrong_path_is_not_a_callback() {
    let tmp = Tmp::new();
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-test-1", "refresh-test-1", "TestType", 100),
    });
    let (port, binder) = hold_port();
    let path = tmp.child("tokens.json");
    let started = start_login(
        login_request(
            &server.token_url(),
            &format!("http://127.0.0.1:{port}/cb"),
            path,
            Duration::from_secs(5),
            Some(ID),
            Some(SECRET),
        ),
        true,
        binder,
        rename_ok,
        CLOCK,
    );
    let authorize = tokio::time::timeout(Duration::from_secs(2), started.opened)
        .await
        .unwrap()
        .unwrap();
    let wrong = send_callback(
        &format!("127.0.0.1:{port}"),
        &format!("/other?state={}&code={CODE}", state_of(&authorize)),
    );
    assert!(server.hits().is_empty());
    assert!(!wrong.contains(CODE));
    assert!(!wrong.contains(SECRET));
    let right = send_callback(
        &format!("127.0.0.1:{port}"),
        &format!("/cb?state={}&code={CODE}", state_of(&authorize)),
    );
    let (result, _) = tokio::time::timeout(Duration::from_secs(5), started.task)
        .await
        .unwrap()
        .unwrap();
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(server.hits().len(), 1);
    assert!(!right.contains(CODE));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_timeout_is_auth_without_sleeping_three_minutes() {
    let tmp = Tmp::new();
    let server = Server::spawn(Reply::Close);
    let (port, binder) = hold_port();
    let started = Instant::now();
    let task = start_login(
        login_request(
            &server.token_url(),
            &format!("http://127.0.0.1:{port}/cb"),
            tmp.child("tokens.json"),
            Duration::from_millis(50),
            Some(ID),
            Some(SECRET),
        ),
        true,
        binder,
        rename_ok,
        CLOCK,
    );
    let _ = tokio::time::timeout(Duration::from_secs(2), task.opened)
        .await
        .unwrap()
        .unwrap();
    let (result, _) = tokio::time::timeout(Duration::from_secs(2), task.task)
        .await
        .unwrap()
        .unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_kind(&result.unwrap_err(), Kind::Auth);
    assert!(server.hits().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn denial_with_error_and_error_reason_is_auth() {
    denial_case("error=access_denied&error_reason=user_denied&state={state}").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn denial_with_only_error_is_auth() {
    denial_case("error=access_denied&state={state}").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn denial_with_only_error_reason_is_auth() {
    denial_case("error_reason=user_denied&state={state}").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn error_with_code_does_not_exchange() {
    denial_case(&format!("error=access_denied&code={CODE}&state={{state}}")).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn foreign_state_is_auth_and_leaves_file() {
    denial_case(&format!("state=foreign-state&code={CODE}")).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_or_empty_state_is_auth() {
    denial_case(&format!("code={CODE}")).await;
    denial_case(&format!("state=&code={CODE}")).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn empty_code_is_auth() {
    denial_case("state={state}&code=").await;
}

async fn denial_case(query: &str) {
    let tmp = Tmp::new();
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-new", "refresh-new", "TestType", 100),
    });
    let path = tmp.child("tokens.json");
    let prior = br#"{"access_token":"access-test-1","refresh_token":"refresh-test-1","token_type":"TestType","expires_at":1}"#;
    std::fs::write(&path, prior).unwrap();
    let (port, binder) = hold_port();
    let started = start_login(
        login_request(
            &server.token_url(),
            &format!("http://127.0.0.1:{port}/cb"),
            path.clone(),
            Duration::from_secs(5),
            Some(ID),
            Some(SECRET),
        ),
        true,
        binder,
        rename_ok,
        CLOCK,
    );
    let authorize = tokio::time::timeout(Duration::from_secs(2), started.opened)
        .await
        .unwrap()
        .unwrap();
    let query = query.replace("{state}", &state_of(&authorize));
    let browser = send_callback(&format!("127.0.0.1:{port}"), &format!("/cb?{query}"));
    let (result, stderr) = tokio::time::timeout(Duration::from_secs(5), started.task)
        .await
        .unwrap()
        .unwrap();
    let err = result.unwrap_err();
    assert_kind(&err, Kind::Auth);
    assert_eq!(std::fs::read(&path).unwrap(), prior);
    assert!(server.hits().is_empty());
    assert!(stderr.is_empty());
    assert!(!browser.contains(CODE));
    assert!(!browser.contains(SECRET));
    assert!(!browser.contains("access-test-1"));
    assert!(browser.contains('а') || browser.contains('Н') || browser.contains('о'));
    assert!(output::render_fail(true, &err).contains("\"kind\":\"auth\""));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn consent_exchanges_and_stops_listener() {
    let tmp = Tmp::new();
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-test-1", "refresh-test-1", "TestType", 100),
    });
    let (port, binder) = hold_port();
    let started = start_login(
        login_request(
            &server.token_url(),
            &format!("http://127.0.0.1:{port}/cb"),
            tmp.child("tokens.json"),
            Duration::from_secs(5),
            Some(ID),
            Some(SECRET),
        ),
        true,
        binder,
        rename_ok,
        CLOCK,
    );
    let authorize = tokio::time::timeout(Duration::from_secs(2), started.opened)
        .await
        .unwrap()
        .unwrap();
    let target = format!("/cb?state={}&code={CODE}", state_of(&authorize));
    let _ = send_callback(&format!("127.0.0.1:{port}"), &target);
    let (result, _) = tokio::time::timeout(Duration::from_secs(5), started.task)
        .await
        .unwrap()
        .unwrap();
    assert!(result.is_ok(), "{result:?}");
    let second = std::net::TcpStream::connect(format!("127.0.0.1:{port}"));
    assert!(second.is_err() || server.hits().len() == 1);
    assert_eq!(server.hits().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn first_login_creates_file_only_after_success() {
    let tmp = Tmp::new();
    let path = tmp.child("nested").join("tokens.json");
    assert!(!path.exists());
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-test-1", "refresh-test-1", "TestType", 100),
    });
    let done = consent_login(&server, path.clone(), None, true).await;
    assert!(done.result.is_ok(), "{:?}", done.result);
    let stored: StoredTokens = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(stored.access_token, "access-test-1");
    assert_eq!(stored.refresh_token, "refresh-test-1");
    assert_eq!(stored.token_type, "TestType");
    assert_eq!(stored.expires_at, CLOCK as i64 + 100);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeat_login_replaces_file_and_failed_login_keeps_bytes() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-1", "refresh-1", "TestType", 100),
    });
    assert!(
        consent_login(&server, path.clone(), None, true)
            .await
            .result
            .is_ok()
    );
    drop(server);
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-2", "refresh-2", "TestType", 100),
    });
    assert!(
        consent_login(&server, path.clone(), None, true)
            .await
            .result
            .is_ok()
    );
    let kept = std::fs::read(&path).unwrap();
    let stored: StoredTokens = serde_json::from_slice(&kept).unwrap();
    assert_eq!(stored.refresh_token, "refresh-2");
    drop(server);
    let server = Server::spawn(Reply::Status {
        status: 400,
        body: token_body("access-3", "refresh-3", "TestType", 100),
    });
    let failed = consent_login(&server, path.clone(), None, true).await;
    assert_kind(&failed.result.unwrap_err(), Kind::Auth);
    assert_eq!(std::fs::read(&path).unwrap(), kept);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refresh_rotates_refresh_token_before_success_message() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_stored(&path, &sample_tokens("refresh-old"));
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-new", "refresh-new", "MAC", 100),
    });
    let http = client_for(&server.token_url(), Duration::from_secs(5));
    let result = auth::run_refresh(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        rename_ok,
    )
    .await;
    let view = result.as_ref().unwrap();
    let stored: StoredTokens = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(stored.refresh_token, "refresh-new");
    assert_eq!(stored.access_token, "access-new");
    assert_eq!(stored.token_type, "MAC");
    assert_eq!(view.expires_at, stored.expires_at);
    let message = output::render_success(false, view);
    assert!(message.contains("Срок:"));
    assert!(message.contains(&path.display().to_string()));
    assert!(!message.contains("refresh-new"));

    write_stored(&path, &sample_tokens("refresh-old"));
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-x", "refresh-x", "TestType", 100),
    });
    let http = client_for(&server.token_url(), Duration::from_secs(5));
    let failed = auth::run_refresh(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        |_: &Path, _: &Path| Err(std::io::Error::other("rename")),
    )
    .await;
    assert!(failed.is_err());
    assert_eq!(
        serde_json::from_slice::<StoredTokens>(&std::fs::read(&path).unwrap())
            .unwrap()
            .refresh_token,
        "refresh-old"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rename_failure_is_io_and_keeps_previous_bytes() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_stored(&path, &sample_tokens("refresh-old"));
    let prior = std::fs::read(&path).unwrap();
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-new", "refresh-new", "TestType", 100),
    });
    let http = client_for(&server.token_url(), Duration::from_secs(5));
    let err = auth::run_refresh(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        |_: &Path, _: &Path| Err(std::io::Error::other("rename")),
    )
    .await
    .unwrap_err();
    assert_kind(&err, Kind::Io);
    assert_eq!(std::fs::read(&path).unwrap(), prior);
    let rendered = output::render_fail(false, &err);
    assert!(!rendered.is_empty());
    for entry in std::fs::read_dir(tmp.path()).unwrap() {
        let entry = entry.unwrap();
        if entry.path() == path {
            continue;
        }
        let bytes = std::fs::read(entry.path()).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains("refresh-new"), "{}", entry.path().display());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_refresh_keeps_previous_bytes() {
    for reply in [
        Reply::Status {
            status: 400,
            body: token_body("CANARY_ACCESS", "refresh-new", "TestType", 100),
        },
        Reply::Close,
    ] {
        let tmp = Tmp::new();
        let path = tmp.child("tokens.json");
        write_stored(&path, &sample_tokens("refresh-old"));
        let prior = std::fs::read(&path).unwrap();
        let server = Server::spawn(reply);
        let http = client_for(&server.token_url(), Duration::from_secs(2));
        let err = auth::run_refresh(
            Some(ID),
            Some(SECRET),
            &path,
            &http,
            fixed_clock(CLOCK),
            rename_ok,
        )
        .await
        .unwrap_err();
        assert!(err.kind() == Kind::Auth || err.kind() == Kind::Api);
        assert_eq!(std::fs::read(&path).unwrap(), prior);
        let stdout = String::new();
        let stderr = output::render_fail(true, &err);
        assert!(stdout.is_empty());
        assert!(!stderr.contains("CANARY_ACCESS"));
        assert!(!format!("{err:?}").contains("CANARY_ACCESS"));
        assert!(!format!("{:?}", sample_tokens("refresh-old")).contains("CANARY_ACCESS"));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn created_directory_is_0700_and_file_is_0600() {
    let tmp = Tmp::new();
    let path = tmp.child("fresh").join("tokens.json");
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-test-1", "refresh-test-1", "TestType", 100),
    });
    assert!(
        consent_login(&server, path.clone(), None, true)
            .await
            .result
            .is_ok()
    );
    let dir_mode = std::fs::metadata(path.parent().unwrap())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    let file_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(dir_mode, 0o700);
    assert_eq!(file_mode, 0o600);
}

#[test]
fn token_path_follows_xdg_then_home_config() {
    let override_path =
        config::token_path(Some("/tmp/custom-tokens.json"), Some("/xdg"), Some("/home")).unwrap();
    assert_eq!(override_path, Path::new("/tmp/custom-tokens.json"));
    let xdg = config::token_path(None, Some("/xdg"), Some("/home")).unwrap();
    assert_eq!(xdg, Path::new("/xdg/boosty-cli/tokens.json"));
    let empty_xdg = config::token_path(Some(""), Some(""), Some("/home")).unwrap();
    assert_eq!(empty_xdg, Path::new("/home/.config/boosty-cli/tokens.json"));
    let home = config::token_path(None, None, Some("/home")).unwrap();
    assert_eq!(home, Path::new("/home/.config/boosty-cli/tokens.json"));
    assert!(!home.display().to_string().contains("Application Support"));
    assert!(config::token_path(None, None, None).is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_bearer_token_type_is_stored_and_no_bearer_is_sent() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-test-1", "refresh-test-1", "MAC", 100),
    });
    assert!(
        consent_login(&server, path.clone(), None, true)
            .await
            .result
            .is_ok()
    );
    let stored: StoredTokens = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(stored.token_type, "MAC");
    drop(server);
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-2", "refresh-2", "MAC", 100),
    });
    let http = client_for(&server.token_url(), Duration::from_secs(5));
    auth::run_refresh(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        rename_ok,
    )
    .await
    .unwrap();
    assert_token_request(&server.hits()[0]);
    assert!(header(&server.hits()[0], "authorization")[0].starts_with("Basic "));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refresh_ignores_expiry() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    let mut tokens = sample_tokens("refresh-old");
    tokens.expires_at = 1;
    write_stored(&path, &tokens);
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-new", "refresh-new", "TestType", 100),
    });
    let http = client_for(&server.token_url(), Duration::from_secs(5));
    auth::run_refresh(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        rename_ok,
    )
    .await
    .unwrap();
    assert_eq!(server.hits().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refresh_without_file_is_auth_and_offline() {
    let tmp = Tmp::new();
    let path = tmp.child("missing.json");
    let server = Server::spawn(Reply::Close);
    let http = client_for(&server.token_url(), Duration::from_secs(2));
    let err = auth::run_refresh(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        rename_ok,
    )
    .await
    .unwrap_err();
    assert_kind(&err, Kind::Auth);
    assert!(server.hits().is_empty());
    assert!(!path.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refresh_without_refresh_token_is_auth_and_offline() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    let prior = br#"{"access_token":"access-test-1","token_type":"TestType","expires_at":1}"#;
    std::fs::write(&path, prior).unwrap();
    let server = Server::spawn(Reply::Close);
    let http = client_for(&server.token_url(), Duration::from_secs(2));
    let err = auth::run_refresh(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        rename_ok,
    )
    .await
    .unwrap_err();
    assert_kind(&err, Kind::Auth);
    assert_eq!(std::fs::read(&path).unwrap(), prior);
    assert!(server.hits().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refresh_unreadable_file_is_io_and_offline() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    std::fs::write(&path, b"not-json").unwrap();
    let server = Server::spawn(Reply::Close);
    let http = client_for(&server.token_url(), Duration::from_secs(2));
    let err = auth::run_refresh(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        rename_ok,
    )
    .await
    .unwrap_err();
    assert_kind(&err, Kind::Io);
    assert_eq!(std::fs::read(&path).unwrap(), b"not-json");
    assert!(server.hits().is_empty());
}

#[test]
fn status_without_file_is_auth_and_offline() {
    let tmp = Tmp::new();
    let server = Server::spawn(Reply::Close);
    let err = crate::auth::run_status(&tmp.child("missing.json")).unwrap_err();
    assert_kind(&err, Kind::Auth);
    assert!(server.hits().is_empty());
}

#[test]
fn status_unreadable_or_incomplete_file_is_io() {
    let tmp = Tmp::new();
    let server = Server::spawn(Reply::Close);
    let broken = tmp.child("broken.json");
    std::fs::write(&broken, b"not-json").unwrap();
    let err = crate::auth::run_status(&broken).unwrap_err();
    assert_kind(&err, Kind::Io);
    assert_eq!(std::fs::read(&broken).unwrap(), b"not-json");
    let incomplete = tmp.child("incomplete.json");
    let prior = br#"{"access_token":"access-test-1"}"#;
    std::fs::write(&incomplete, prior).unwrap();
    let err = crate::auth::run_status(&incomplete).unwrap_err();
    assert_kind(&err, Kind::Io);
    assert_eq!(std::fs::read(&incomplete).unwrap(), prior);
    assert!(server.hits().is_empty());
}

#[test]
fn status_prints_expiry_and_path_without_tokens_or_network() {
    let tmp = Tmp::new();
    let server = Server::spawn(Reply::Close);
    let path = tmp.child("tokens.json");
    write_stored(&path, &sample_tokens("refresh-test-1"));
    let view = crate::auth::run_status(&path).unwrap();
    let text = output::render_success(false, &view);
    assert!(text.contains("Срок:"));
    assert!(text.contains(&path.display().to_string()));
    assert!(!text.contains("access-test-1"));
    assert!(!text.contains("refresh-test-1"));
    assert!(server.hits().is_empty());
    let proc = run_bin(
        &["auth", "status"],
        &[
            ("HOME", tmp.path().to_str().unwrap()),
            ("BOOSTY_TOKEN_PATH", path.to_str().unwrap()),
            ("BOOSTY_TOKEN_URL", &server.token_url()),
            ("BOOSTY_DEVAPI_BASE", &server.base()),
            ("TZ", "UTC"),
        ],
    );
    assert_eq!(proc.code, 0, "{}", proc.stderr);
    assert!(server.hits().is_empty());
    assert!(proc.stdout.contains("Срок:"));
    assert!(!proc.stdout.contains("access-test-1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_client_secret_on_refresh_is_auth() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_stored(&path, &sample_tokens("refresh-old"));
    let prior = std::fs::read(&path).unwrap();
    let server = Server::spawn(Reply::Close);
    let http = client_for(&server.token_url(), Duration::from_secs(2));
    let err = auth::run_refresh(Some(ID), None, &path, &http, fixed_clock(CLOCK), rename_ok)
        .await
        .unwrap_err();
    assert_kind(&err, Kind::Auth);
    assert!(server.hits().is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), prior);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_success_has_expires_at_and_path_only() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-test-1", "refresh-test-1", "TestType", 100),
    });
    let login = consent_login(&server, path.clone(), None, true).await;
    assert_json_view(login.result.unwrap(), &path);
    drop(server);
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: token_body("access-2", "refresh-2", "TestType", 50),
    });
    let http = client_for(&server.token_url(), Duration::from_secs(5));
    let view = auth::run_refresh(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        rename_ok,
    )
    .await
    .unwrap();
    assert_json_view(view, &path);
    let status = crate::auth::run_status(&path).unwrap();
    assert_json_view(status, &path);
}

fn assert_json_view(view: SessionView, path: &Path) {
    let text = output::render_success(true, &view);
    assert_eq!(text.lines().count(), 1);
    let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
    let keys: Vec<_> = value.as_object().unwrap().keys().cloned().collect();
    let mut keys = keys;
    keys.sort();
    assert_eq!(keys, vec!["expires_at".to_string(), "path".to_string()]);
    let stored: StoredTokens = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(value["expires_at"], stored.expires_at);
    assert!(!text.contains("access-test-1"));
    assert!(!text.contains(SECRET));
    assert!(!text.contains(CODE));
}

#[test]
fn debug_does_not_contain_token_values() {
    let tokens = StoredTokens {
        access_token: "CANARY_ACCESS".to_string(),
        refresh_token: "CANARY_REFRESH".to_string(),
        token_type: "TestType".to_string(),
        expires_at: 1,
    };
    let debug = format!("{tokens:?}");
    assert!(!debug.contains("CANARY_ACCESS"));
    assert!(!debug.contains("CANARY_REFRESH"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_error_is_one_object_and_human_error_is_russian() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_stored(&path, &sample_tokens("refresh-old"));
    let server = Server::spawn(Reply::Status {
        status: 400,
        body: r#"{"access_token":"CANARY_ACCESS","note":"CANARY_BODY"}"#.to_string(),
    });
    let http = client_for(&server.token_url(), Duration::from_secs(5));
    let err = auth::run_refresh(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        rename_ok,
    )
    .await
    .unwrap_err();
    assert_kind(&err, Kind::Auth);
    let json = output::render_fail(true, &err);
    let parsed: serde_json::Value = serde_json::from_str(json.trim()).unwrap();
    assert_eq!(parsed.as_object().unwrap().len(), 2);
    assert_eq!(parsed["kind"], "auth");
    assert!(
        !parsed["error"]
            .as_str()
            .unwrap()
            .chars()
            .any(|ch| ('а'..='я').contains(&ch) || ('А'..='Я').contains(&ch))
    );
    let human = output::render_fail(false, &err);
    assert!(human.contains('а') || human.contains('Н'));
    assert!(serde_json::from_str::<serde_json::Value>(human.trim()).is_err());
    assert!(!json.contains("CANARY_ACCESS"));
    assert!(!human.contains("CANARY_ACCESS"));
    assert!(!format!("{err:?}").contains("CANARY_ACCESS"));
    assert!(!format!("{:?}", sample_tokens("refresh-old")).contains("CANARY_ACCESS"));
    let proc = run_bin(
        &["--json", "auth", "refresh"],
        &[
            ("HOME", tmp.path().to_str().unwrap()),
            ("BOOSTY_TOKEN_PATH", path.to_str().unwrap()),
            ("BOOSTY_CLIENT_ID", ID),
            ("BOOSTY_CLIENT_SECRET", SECRET),
            ("BOOSTY_TOKEN_URL", &server.token_url()),
        ],
    );
    assert_eq!(proc.code, 3);
    assert!(proc.stdout.is_empty());
    assert!(!proc.stderr.contains("CANARY_ACCESS"));
    assert!(!proc.stderr.contains("CANARY_BODY"));
    let one: serde_json::Value = serde_json::from_str(proc.stderr.trim()).unwrap();
    assert_eq!(one["kind"], "auth");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_endpoint_4xx_on_exchange_is_auth() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    let server = Server::spawn(Reply::Status {
        status: 400,
        body: token_body("access-test-1", "refresh-test-1", "TestType", 100),
    });
    let done = consent_login(&server, path.clone(), None, true).await;
    assert_kind(&done.result.unwrap_err(), Kind::Auth);
    assert!(!path.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_endpoint_4xx_on_refresh_is_auth() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_stored(&path, &sample_tokens("refresh-old"));
    let prior = std::fs::read(&path).unwrap();
    let server = Server::spawn(Reply::Status {
        status: 400,
        body: token_body("access-test-1", "refresh-new", "TestType", 100),
    });
    let http = client_for(&server.token_url(), Duration::from_secs(5));
    let err = auth::run_refresh(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        rename_ok,
    )
    .await
    .unwrap_err();
    assert_kind(&err, Kind::Auth);
    assert_eq!(std::fs::read(&path).unwrap(), prior);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_endpoint_5xx_is_api() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    let server = Server::spawn(Reply::Status {
        status: 500,
        body: "nope".to_string(),
    });
    let done = consent_login(&server, path.clone(), None, true).await;
    assert_kind(&done.result.unwrap_err(), Kind::Api);
    assert!(!path.exists());
    write_stored(&path, &sample_tokens("refresh-old"));
    let prior = std::fs::read(&path).unwrap();
    drop(server);
    let server = Server::spawn(Reply::Status {
        status: 500,
        body: "nope".to_string(),
    });
    let http = client_for(&server.token_url(), Duration::from_secs(5));
    let err = auth::run_refresh(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        rename_ok,
    )
    .await
    .unwrap_err();
    assert_kind(&err, Kind::Api);
    assert_eq!(std::fs::read(&path).unwrap(), prior);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_endpoint_disconnect_is_api() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_stored(&path, &sample_tokens("refresh-old"));
    let prior = std::fs::read(&path).unwrap();
    let server = Server::spawn(Reply::Close);
    let http = client_for(&server.token_url(), Duration::from_secs(5));
    let err = auth::run_refresh(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        rename_ok,
    )
    .await
    .unwrap_err();
    assert_kind(&err, Kind::Api);
    assert_eq!(std::fs::read(&path).unwrap(), prior);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_endpoint_non_json_is_api() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_stored(&path, &sample_tokens("refresh-old"));
    let prior = std::fs::read(&path).unwrap();
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: "not-json".to_string(),
    });
    let http = client_for(&server.token_url(), Duration::from_secs(5));
    let err = auth::run_refresh(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        rename_ok,
    )
    .await
    .unwrap_err();
    assert_kind(&err, Kind::Api);
    assert_eq!(std::fs::read(&path).unwrap(), prior);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_endpoint_missing_field_is_api() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_stored(&path, &sample_tokens("refresh-old"));
    let prior = std::fs::read(&path).unwrap();
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: r#"{"access_token":"access-test-1","expires_in":100,"token_type":"TestType"}"#
            .to_string(),
    });
    let http = client_for(&server.token_url(), Duration::from_secs(5));
    let err = auth::run_refresh(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        rename_ok,
    )
    .await
    .unwrap_err();
    assert_kind(&err, Kind::Api);
    assert_eq!(std::fs::read(&path).unwrap(), prior);
}

use std::os::unix::fs::PermissionsExt;
