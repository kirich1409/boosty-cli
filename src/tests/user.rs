use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::suite::{collect_strings, json_strings};
use super::support::*;
use crate::api::user::UserResponse;
use crate::client;
use crate::config::StoredTokens;
use crate::error::{Fail, Kind};
use crate::output;

const ACCESS_OLD: &str = "access-old-7k2";
const ACCESS_NEW: &str = "access-new-7k2";
const REFRESH_OLD: &str = "refresh-old-7k2";
const REFRESH_NEW: &str = "refresh-new-7k2";
const EMAIL: &str = "nika.fixture@invalid.test";
const EMAIL_CANARY: &str = "canary-mail-7k2@invalid.test";
const ACCESS_CANARY: &str = "canary-access-7k2";
const BODY_CANARY: &str = "canary-body-7k2";
const API_KEY: &str = "fixture-api-key-7k2";
const FAR_EXPIRES: i64 = 4_000_000_000;

fn user_fixture() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/user.json"))
        .unwrap()
}

fn contracts() -> (String, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    (
        std::fs::read_to_string(root.join("devapi.md")).unwrap(),
        std::fs::read_to_string(root.join("authorization_code_flow.md")).unwrap(),
    )
}

fn write_bearer(path: &Path, access: &str, refresh: &str, token_type: &str, expires_at: i64) {
    write_stored(
        path,
        &StoredTokens {
            access_token: access.to_string(),
            refresh_token: refresh.to_string(),
            token_type: token_type.to_string(),
            expires_at,
        },
    );
}

fn devapi_base_of(server: &Server) -> String {
    let base = format!("{}/v1", server.base());
    assert!(!base.ends_with('/'), "{base}");
    base
}

fn http_for(token: &Server, devapi: &Server, timeout: Duration) -> crate::client::ApiClient {
    client_for_bases(&token.token_url(), &devapi_base_of(devapi), timeout)
}

fn remove_at(value: &serde_json::Value, path: &[&str]) -> serde_json::Value {
    let mut clone = value.clone();
    let mut cursor = &mut clone;
    for (index, key) in path.iter().enumerate() {
        if index + 1 == path.len() {
            cursor.as_object_mut().unwrap().remove(*key);
            return clone;
        }
        cursor = cursor.get_mut(*key).unwrap();
    }
    clone
}

fn assert_user_get(hit: &Hit, token: &str) {
    assert_eq!(hit.method, "GET");
    assert_eq!(hit.path, "/v1/user");
    assert!(hit.query.is_empty(), "{}", hit.query);
    let auths = header(hit, "authorization");
    assert_eq!(auths.len(), 1, "{:?}", hit.headers);
    assert_eq!(auths[0], format!("Bearer {token}"));
    assert!(!auths[0].starts_with("Basic"));
    assert!(hit.headers.iter().all(|(key, _)| key != "x-api-key"));
    assert_eq!(header(hit, "accept"), vec!["application/json"]);
    assert_eq!(
        header(hit, "user-agent"),
        vec![client::user_agent().as_str()]
    );
}

fn assert_refresh_post(hit: &Hit, refresh: &str) {
    assert_token_request(hit);
    let pairs = form_pairs(&hit.body);
    assert_eq!(pairs.len(), 2, "{}", hit.body);
    assert!(
        pairs
            .iter()
            .any(|(key, value)| key == "grant_type" && value == "refresh_token")
    );
    assert!(
        pairs
            .iter()
            .any(|(key, value)| key == "refresh_token" && value == refresh)
    );
}

fn assert_no_canary(err: &Fail, canaries: &[&str]) {
    let debug = format!("{err:?}");
    for canary in canaries {
        assert!(!err.english().contains(canary), "{canary}");
        assert!(!err.russian().contains(canary), "{canary}");
        assert!(!debug.contains(canary), "{canary} {debug}");
    }
}

fn canary_body() -> String {
    format!(r#"{{"email":"{EMAIL_CANARY}","access":"{ACCESS_CANARY}","note":"{BODY_CANARY}"}}"#)
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

fn assert_json_fail(stderr: &str, kind: &str, canaries: &[&str]) {
    assert_eq!(stderr.trim().lines().count(), 1, "{stderr}");
    assert!(!stderr.trim().is_empty());
    let value: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    let mut keys: Vec<_> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, vec!["error".to_string(), "kind".to_string()]);
    assert_eq!(value["kind"], kind);
    let error = value["error"].as_str().unwrap();
    assert!(!error.is_empty());
    assert!(error.is_ascii());
    assert!(
        !error
            .chars()
            .any(|ch| ('а'..='я').contains(&ch) || ('А'..='Я').contains(&ch))
    );
    for canary in canaries {
        assert!(!stderr.contains(canary), "{canary} {stderr}");
        assert!(!error.contains(canary), "{canary}");
    }
}

fn assert_russian_fail(stderr: &str, canaries: &[&str]) {
    assert!(stderr.contains('а') || stderr.contains('Н'), "{stderr}");
    assert!(serde_json::from_str::<serde_json::Value>(stderr.trim()).is_err());
    for canary in canaries {
        assert!(!stderr.contains(canary), "{canary}");
    }
}

#[test]
fn user_fixture_decodes_and_drops_unknown_field() {
    let parsed: UserResponse = serde_json::from_str(&user_fixture()).unwrap();
    assert_eq!(parsed.user.id, 3_000_000_001_i64);
    let again = serde_json::to_value(&parsed).unwrap();
    let text = again.to_string();
    assert!(!text.contains("ignoredField"), "{text}");
    assert!(!text.contains("fixture-extra-nika-7k2"), "{text}");
    assert!(!text.contains("null"), "{text}");
    assert!(again.get("user").is_some());
    for key in ["id", "name", "email", "avatarUrl", "flags"] {
        assert!(again["user"].get(key).is_some(), "{key}");
        assert!(!again["user"][key].is_null(), "{key}");
    }
    for key in ["isAuthor", "isBanned"] {
        assert!(again["user"]["flags"].get(key).is_some(), "{key}");
        assert!(!again["user"]["flags"][key].is_null(), "{key}");
    }
    assert_eq!(again["user"]["id"], 3_000_000_001_i64);
    assert!(again["user"]["id"].is_i64());
    assert!(again["user"]["flags"]["isAuthor"].is_boolean());
    assert!(again["user"]["flags"]["isBanned"].is_boolean());
}

#[test]
fn user_fixture_missing_or_wrong_type_is_an_error() {
    let value: serde_json::Value = serde_json::from_str(&user_fixture()).unwrap();
    for path in [
        &["user"][..],
        &["user", "id"],
        &["user", "name"],
        &["user", "email"],
        &["user", "avatarUrl"],
        &["user", "flags"],
        &["user", "flags", "isAuthor"],
        &["user", "flags", "isBanned"],
    ] {
        let broken = remove_at(&value, path);
        assert!(
            serde_json::from_value::<UserResponse>(broken).is_err(),
            "{path:?}"
        );
    }
    let mut id_string = value.clone();
    id_string["user"]["id"] = serde_json::json!("3000000001");
    assert!(serde_json::from_value::<UserResponse>(id_string).is_err());
    let mut flag_string = value.clone();
    flag_string["user"]["flags"]["isAuthor"] = serde_json::json!("true");
    assert!(serde_json::from_value::<UserResponse>(flag_string).is_err());
    let mut flag_one = value.clone();
    flag_one["user"]["flags"]["isBanned"] = serde_json::json!(1);
    assert!(serde_json::from_value::<UserResponse>(flag_one).is_err());
    let mut flag_zero = value.clone();
    flag_zero["user"]["flags"]["isAuthor"] = serde_json::json!(0);
    assert!(serde_json::from_value::<UserResponse>(flag_zero).is_err());
}

#[test]
fn user_fixture_does_not_copy_contract_samples() {
    let fixture = user_fixture();
    let (devapi, flow) = contracts();
    let parsed: serde_json::Value = serde_json::from_str(&fixture).unwrap();
    let mut fixture_strings = Vec::new();
    collect_strings(&parsed, &mut fixture_strings);
    for value in &fixture_strings {
        assert!(!devapi.contains(value), "{value}");
        assert!(!flow.contains(value), "{value}");
    }
    for contract in [&devapi, &flow] {
        for sample in json_strings(contract) {
            assert!(!fixture.contains(&sample), "{sample}");
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_get_sends_one_bearer_without_query() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_bearer(
        path.as_path(),
        ACCESS_OLD,
        REFRESH_OLD,
        "Bearer",
        CLOCK as i64 + 61,
    );
    let prior = std::fs::read(&path).unwrap();
    let token = Server::spawn(Reply::Close);
    let devapi = Server::spawn_made(|base| {
        let avatar = format!("{base}/avatar-side");
        let mut body: serde_json::Value = serde_json::from_str(&user_fixture()).unwrap();
        body["user"]["avatarUrl"] = serde_json::Value::String(avatar);
        Reply::Status {
            status: 200,
            body: body.to_string(),
        }
    });
    let avatar = format!("{}/avatar-side", devapi.base());
    assert_ne!(avatar, format!("{}/v1/user", devapi.base()));
    let http = http_for(&token, &devapi, Duration::from_secs(5));
    let parsed = crate::user::run_user(None, None, &path, &http, fixed_clock(CLOCK), rename_ok)
        .await
        .unwrap();
    assert_eq!(devapi.hits().len(), 1);
    assert_user_get(&devapi.hits()[0], ACCESS_OLD);
    assert!(token.hits().is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), prior);
    assert_eq!(parsed.user.avatarUrl, avatar);
    assert!(devapi.hits().iter().all(|hit| hit.path == "/v1/user"));
}

#[test]
fn user_binary_ignores_api_key_env() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", FAR_EXPIRES);
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: user_fixture(),
    });
    let base = devapi_base_of(&server);
    let home = tmp.path().to_str().unwrap().to_string();
    let path_s = path.to_str().unwrap().to_string();
    let token_url = server.token_url();
    let out = run_bin(
        &["user"],
        &[
            ("HOME", home.as_str()),
            ("BOOSTY_TOKEN_PATH", path_s.as_str()),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", token_url.as_str()),
            ("BOOSTY_API_KEY", API_KEY),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(server.hits().len(), 1);
    let hit = &server.hits()[0];
    assert_user_get(hit, ACCESS_OLD);
    assert!(
        hit.headers
            .iter()
            .all(|(_, value)| !value.contains(API_KEY))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_refresh_window_boundary_on_fixed_clock() {
    for (expires_at, refresh) in [
        (CLOCK as i64 + 60, true),
        (CLOCK as i64 - 1, true),
        (CLOCK as i64 + 61, false),
    ] {
        let tmp = Tmp::new();
        let path = tmp.child("tokens.json");
        write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", expires_at);
        let prior = std::fs::read(&path).unwrap();
        let token = Server::spawn(Reply::Status {
            status: 200,
            body: token_body(ACCESS_NEW, REFRESH_NEW, "Bearer", 100),
        });
        let devapi = Server::spawn(Reply::Status {
            status: 200,
            body: user_fixture(),
        });
        let http = http_for(&token, &devapi, Duration::from_secs(5));
        let (id, secret) = if refresh {
            (Some(ID), Some(SECRET))
        } else {
            (None, None)
        };
        crate::user::run_user(id, secret, &path, &http, fixed_clock(CLOCK), rename_ok)
            .await
            .unwrap();
        assert_eq!(devapi.hits().len(), 1, "expires_at={expires_at}");
        if refresh {
            assert_eq!(token.hits().len(), 1, "expires_at={expires_at}");
            assert_refresh_post(&token.hits()[0], REFRESH_OLD);
            assert_user_get(&devapi.hits()[0], ACCESS_NEW);
            assert!(!header(&devapi.hits()[0], "authorization")[0].contains(ACCESS_OLD));
        } else {
            assert!(token.hits().is_empty(), "expires_at={expires_at}");
            assert_eq!(std::fs::read(&path).unwrap(), prior);
            assert_user_get(&devapi.hits()[0], ACCESS_OLD);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_refresh_writes_new_token_before_devapi_and_success_text() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", CLOCK as i64);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_hook = Arc::clone(&seen);
    let path_hook = path.clone();
    let token = Server::spawn(Reply::Status {
        status: 200,
        body: token_body(ACCESS_NEW, REFRESH_NEW, "Bearer", 100),
    });
    let devapi = Server::spawn_before_reply(
        Reply::Status {
            status: 200,
            body: user_fixture(),
        },
        move || {
            let bytes = std::fs::read(&path_hook).unwrap_or_default();
            *seen_hook.lock().unwrap() = bytes;
        },
    );
    let http = http_for(&token, &devapi, Duration::from_secs(5));
    let body = crate::user::run_user(
        Some(ID),
        Some(SECRET),
        &path,
        &http,
        fixed_clock(CLOCK),
        rename_ok,
    )
    .await
    .unwrap();
    let snap = String::from_utf8(seen.lock().unwrap().clone()).unwrap();
    assert!(snap.contains(REFRESH_NEW), "{snap}");
    assert!(!snap.contains(REFRESH_OLD), "{snap}");
    let human = output::render_user(false, &body);
    let json = output::render_user(true, &body);
    assert!(!human.contains(REFRESH_NEW), "{human}");
    assert!(!human.contains(ACCESS_NEW), "{human}");
    assert!(!json.contains(REFRESH_NEW), "{json}");
    assert!(!json.contains(ACCESS_NEW), "{json}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_refresh_rename_failure_is_io_and_does_not_call_devapi() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", CLOCK as i64);
    let prior = std::fs::read(&path).unwrap();
    let token = Server::spawn(Reply::Status {
        status: 200,
        body: token_body(ACCESS_NEW, REFRESH_NEW, "Bearer", 100),
    });
    let devapi = Server::spawn(Reply::Status {
        status: 200,
        body: user_fixture(),
    });
    let http = http_for(&token, &devapi, Duration::from_secs(5));
    let err = crate::user::run_user(
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
    assert_eq!(err.code(), 5);
    assert_eq!(std::fs::read(&path).unwrap(), prior);
    assert!(devapi.hits().is_empty());
    for entry in std::fs::read_dir(tmp.path()).unwrap() {
        let entry = entry.unwrap();
        if entry.path() == path {
            continue;
        }
        let bytes = std::fs::read(entry.path()).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains(REFRESH_NEW), "{}", entry.path().display());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_refresh_4xx_is_auth_and_skips_devapi() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", CLOCK as i64);
    let prior = std::fs::read(&path).unwrap();
    let token = Server::spawn(Reply::Status {
        status: 400,
        body: canary_body(),
    });
    let devapi = Server::spawn(Reply::Status {
        status: 200,
        body: user_fixture(),
    });
    let http = http_for(&token, &devapi, Duration::from_secs(5));
    let err = crate::user::run_user(
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
    assert_eq!(err.code(), 3);
    assert_eq!(std::fs::read(&path).unwrap(), prior);
    assert!(devapi.hits().is_empty());
    assert_eq!(token.hits().len(), 1);
    assert_no_canary(&err, &[EMAIL_CANARY, ACCESS_CANARY, BODY_CANARY]);

    let proc_token = Server::spawn(Reply::Status {
        status: 400,
        body: canary_body(),
    });
    let proc_devapi = Server::spawn(Reply::Status {
        status: 200,
        body: user_fixture(),
    });
    let home = tmp.path().to_str().unwrap().to_string();
    let path_s = path.to_str().unwrap().to_string();
    let token_url = proc_token.token_url();
    let base = devapi_base_of(&proc_devapi);
    let env = [
        ("HOME", home.as_str()),
        ("BOOSTY_TOKEN_PATH", path_s.as_str()),
        ("BOOSTY_TOKEN_URL", token_url.as_str()),
        ("BOOSTY_DEVAPI_BASE", base.as_str()),
        ("BOOSTY_CLIENT_ID", ID),
        ("BOOSTY_CLIENT_SECRET", SECRET),
    ];
    write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", 1);
    let prior = std::fs::read(&path).unwrap();
    let mut token_hits = 0;
    for args in [vec!["--json", "user"], vec!["user"]] {
        let out = run_bin(&args, &env);
        assert_eq!(out.code, 3, "{} {}", args.join(" "), out.stderr);
        assert_eq!(out.stdout, "");
        if args.contains(&"--json") {
            assert_json_fail(
                &out.stderr,
                "auth",
                &[EMAIL_CANARY, ACCESS_CANARY, BODY_CANARY],
            );
        } else {
            assert_russian_fail(&out.stderr, &[EMAIL_CANARY, ACCESS_CANARY, BODY_CANARY]);
        }
        let hits = proc_token.hits().len();
        assert_eq!(hits, token_hits + 1, "refresh repeated");
        token_hits = hits;
    }
    assert!(proc_devapi.hits().is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), prior);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_refresh_transport_and_bad_body_stay_api() {
    let replies = [
        Reply::Close,
        Reply::Status {
            status: 200,
            body: "not-json".to_string(),
        },
        Reply::Status {
            status: 200,
            body: r#"{"access_token":"access-test-1","expires_in":100,"token_type":"Bearer"}"#
                .to_string(),
        },
    ];
    for reply in replies {
        let tmp = Tmp::new();
        let path = tmp.child("tokens.json");
        write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", CLOCK as i64);
        let prior = std::fs::read(&path).unwrap();
        let token = Server::spawn(reply);
        let devapi = Server::spawn(Reply::Status {
            status: 200,
            body: user_fixture(),
        });
        let http = http_for(&token, &devapi, Duration::from_secs(5));
        let err = crate::user::run_user(
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
        assert_eq!(err.code(), 4);
        assert_eq!(std::fs::read(&path).unwrap(), prior);
        assert!(devapi.hits().is_empty());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_refresh_missing_credentials_is_auth_without_network() {
    for (id, secret) in [(None, Some(SECRET)), (Some(ID), None), (None, None)] {
        let tmp = Tmp::new();
        let path = tmp.child("tokens.json");
        write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", CLOCK as i64);
        let prior = std::fs::read(&path).unwrap();
        let token = Server::spawn(Reply::Close);
        let devapi = Server::spawn(Reply::Close);
        let http = http_for(&token, &devapi, Duration::from_secs(2));
        let err = crate::user::run_user(id, secret, &path, &http, fixed_clock(CLOCK), rename_ok)
            .await
            .unwrap_err();
        assert_kind(&err, Kind::Auth);
        assert_eq!(err.code(), 3);
        assert!(token.hits().is_empty());
        assert!(devapi.hits().is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), prior);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_missing_token_file_is_auth_and_offline() {
    let tmp = Tmp::new();
    let path = tmp.child("missing.json");
    let token = Server::spawn(Reply::Close);
    let devapi = Server::spawn(Reply::Close);
    let http = http_for(&token, &devapi, Duration::from_secs(2));
    let err = crate::user::run_user(
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
    assert_eq!(err.code(), 3);
    assert!(token.hits().is_empty());
    assert!(devapi.hits().is_empty());
    assert!(!path.exists());

    let server = Server::spawn(Reply::Close);
    let home = tmp.path().to_str().unwrap().to_string();
    let path_s = path.to_str().unwrap().to_string();
    let base = devapi_base_of(&server);
    let token_url = server.token_url();
    let env = [
        ("HOME", home.as_str()),
        ("BOOSTY_TOKEN_PATH", path_s.as_str()),
        ("BOOSTY_DEVAPI_BASE", base.as_str()),
        ("BOOSTY_TOKEN_URL", token_url.as_str()),
    ];
    for args in [vec!["user"], vec!["--json", "user"]] {
        let out = run_bin(&args, &env);
        assert_eq!(out.code, 3, "{} {}", args.join(" "), out.stderr);
        assert_eq!(out.stdout, "");
        if args.contains(&"--json") {
            assert_json_fail(&out.stderr, "auth", &[]);
        }
    }
    assert!(server.hits().is_empty());
    assert!(!path.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_bad_file_is_io_and_offline() {
    let bodies = [
        b"not-json".to_vec(),
        br#"{"access_token":"access-old-7k2","token_type":"Bearer","expires_at":1}"#.to_vec(),
        br#"{"refresh_token":"refresh-old-7k2","token_type":"Bearer","expires_at":1}"#.to_vec(),
    ];
    for bytes in bodies {
        let tmp = Tmp::new();
        let path = tmp.child("tokens.json");
        std::fs::write(&path, &bytes).unwrap();
        let token = Server::spawn(Reply::Close);
        let devapi = Server::spawn(Reply::Close);
        let http = http_for(&token, &devapi, Duration::from_secs(2));
        let err = crate::user::run_user(
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
        assert_eq!(err.code(), 5);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(token.hits().is_empty());
        assert!(devapi.hits().is_empty());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_non_bearer_outside_window_is_auth_and_offline() {
    for token_type in ["bearer", "MAC"] {
        let tmp = Tmp::new();
        let path = tmp.child("tokens.json");
        write_bearer(
            &path,
            ACCESS_OLD,
            REFRESH_OLD,
            token_type,
            CLOCK as i64 + 61,
        );
        let prior = std::fs::read(&path).unwrap();
        let token = Server::spawn(Reply::Close);
        let devapi = Server::spawn(Reply::Close);
        let http = http_for(&token, &devapi, Duration::from_secs(2));
        let err = crate::user::run_user(None, None, &path, &http, fixed_clock(CLOCK), rename_ok)
            .await
            .unwrap_err();
        assert_kind(&err, Kind::Auth);
        assert_eq!(err.code(), 3, "{token_type}");
        assert!(token.hits().is_empty());
        assert!(devapi.hits().is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), prior);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_non_bearer_after_refresh_keeps_file_and_skips_devapi() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", CLOCK as i64);
    let token = Server::spawn(Reply::Status {
        status: 200,
        body: token_body(ACCESS_NEW, REFRESH_NEW, "bearer", 100),
    });
    let devapi = Server::spawn(Reply::Status {
        status: 200,
        body: user_fixture(),
    });
    let http = http_for(&token, &devapi, Duration::from_secs(5));
    let err = crate::user::run_user(
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
    assert_eq!(err.code(), 3);
    let stored: StoredTokens = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(stored.refresh_token, REFRESH_NEW);
    assert_ne!(stored.refresh_token, REFRESH_OLD);
    assert!(devapi.hits().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_parses_json_without_content_type() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", CLOCK as i64 + 61);
    let body = user_fixture();
    let token = Server::spawn(Reply::Close);
    let devapi = Server::spawn(Reply::NoContentType {
        status: 200,
        body: body.clone(),
    });
    let http = http_for(&token, &devapi, Duration::from_secs(5));
    let parsed = crate::user::run_user(None, None, &path, &http, fixed_clock(CLOCK), rename_ok)
        .await
        .unwrap();
    let expected: UserResponse = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed, expected);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_devapi_failure_is_api_without_retry_or_rollback() {
    let canaries = [EMAIL_CANARY, ACCESS_CANARY, BODY_CANARY];
    let outside = [
        Reply::Status {
            status: 401,
            body: canary_body(),
        },
        Reply::Status {
            status: 500,
            body: canary_body(),
        },
        Reply::Close,
        Reply::Status {
            status: 200,
            body: format!("not-json {EMAIL_CANARY} {ACCESS_CANARY}"),
        },
        Reply::Status {
            status: 200,
            body: format!(
                r#"{{"user":{{"name":"Nika","email":"{EMAIL_CANARY}","note":"{ACCESS_CANARY}"}}}}"#
            ),
        },
    ];
    for reply in outside {
        let tmp = Tmp::new();
        let path = tmp.child("tokens.json");
        write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", CLOCK as i64 + 61);
        let prior = std::fs::read(&path).unwrap();
        let token = Server::spawn(Reply::Close);
        let devapi = Server::spawn(reply);
        let http = http_for(&token, &devapi, Duration::from_secs(5));
        let err = crate::user::run_user(None, None, &path, &http, fixed_clock(CLOCK), rename_ok)
            .await
            .unwrap_err();
        assert_kind(&err, Kind::Api);
        assert_eq!(err.code(), 4);
        assert_ne!(err.code(), 3);
        assert!(devapi.hits().len() <= 1);
        assert!(token.hits().is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), prior);
        assert_no_canary(&err, &canaries);
    }

    for reply in [
        Reply::Status {
            status: 500,
            body: canary_body(),
        },
        Reply::Status {
            status: 200,
            body: format!(r#"{{"user":{{"email":"{EMAIL_CANARY}","note":"{ACCESS_CANARY}"}}}}"#),
        },
    ] {
        let tmp = Tmp::new();
        let path = tmp.child("tokens.json");
        write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", CLOCK as i64);
        let token = Server::spawn(Reply::Status {
            status: 200,
            body: token_body(ACCESS_NEW, REFRESH_NEW, "Bearer", 100),
        });
        let devapi = Server::spawn(reply);
        let http = http_for(&token, &devapi, Duration::from_secs(5));
        let err = crate::user::run_user(
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
        assert_eq!(devapi.hits().len(), 1);
        assert_eq!(token.hits().len(), 1);
        let stored: StoredTokens = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(stored.refresh_token, REFRESH_NEW);
        assert_no_canary(&err, &canaries);
    }
}

#[test]
fn user_devapi_error_process_streams() {
    let canaries = [EMAIL_CANARY, ACCESS_CANARY, BODY_CANARY];
    let replies = [
        Reply::Status {
            status: 500,
            body: canary_body(),
        },
        Reply::Status {
            status: 200,
            body: format!(
                r#"{{"user":{{"name":"Nika","email":"{EMAIL_CANARY}","note":"{ACCESS_CANARY}"}}}}"#
            ),
        },
    ];
    for reply in replies {
        let tmp = Tmp::new();
        let path = tmp.child("tokens.json");
        write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", FAR_EXPIRES);
        let server = Server::spawn(reply);
        let home = tmp.path().to_str().unwrap().to_string();
        let path_s = path.to_str().unwrap().to_string();
        let base = devapi_base_of(&server);
        let token_url = server.token_url();
        let env = [
            ("HOME", home.as_str()),
            ("BOOSTY_TOKEN_PATH", path_s.as_str()),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", token_url.as_str()),
        ];
        for args in [vec!["--json", "user"], vec!["user", "--json"], vec!["user"]] {
            let out = run_bin(&args, &env);
            assert_eq!(out.code, 4, "{} {}", args.join(" "), out.stderr);
            assert_eq!(out.stdout, "");
            if args.contains(&"--json") {
                assert_json_fail(&out.stderr, "api", &canaries);
            } else {
                assert_russian_fail(&out.stderr, &canaries);
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_devapi_timeout_is_api_without_retry() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", CLOCK as i64 + 61);
    let token = Server::spawn(Reply::Close);
    let devapi = Server::spawn(Reply::Silent);
    let http = http_for(&token, &devapi, Duration::from_millis(200));
    let started = Instant::now();
    let err = crate::user::run_user(None, None, &path, &http, fixed_clock(CLOCK), rename_ok)
        .await
        .unwrap_err();
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_kind(&err, Kind::Api);
    assert!(devapi.hits().len() <= 1);
    assert!(token.hits().is_empty());
}

#[test]
fn user_json_stdout_is_one_spec_object() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", FAR_EXPIRES);
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: user_fixture(),
    });
    let home = tmp.path().to_str().unwrap().to_string();
    let path_s = path.to_str().unwrap().to_string();
    let base = devapi_base_of(&server);
    let token_url = server.token_url();
    let env = [
        ("HOME", home.as_str()),
        ("BOOSTY_TOKEN_PATH", path_s.as_str()),
        ("BOOSTY_DEVAPI_BASE", base.as_str()),
        ("BOOSTY_TOKEN_URL", token_url.as_str()),
    ];
    for args in [vec!["--json", "user"], vec!["user", "--json"]] {
        let out = run_bin(&args, &env);
        assert_eq!(out.code, 0, "{}", out.stderr);
        assert!(out.stderr.is_empty(), "{}", out.stderr);
        assert!(out.stdout.ends_with('\n'));
        assert!(!out.stdout[..out.stdout.len() - 1].contains('\n'));
        let value: serde_json::Value = serde_json::from_str(out.stdout.trim()).unwrap();
        assert!(value.is_object());
        assert_eq!(value["user"]["name"], "Nika Testova");
        assert_eq!(value["user"]["email"], EMAIL);
        assert_eq!(value["user"]["id"], 3_000_000_001_i64);
        assert!(value["user"]["id"].is_i64());
        assert!(!value["user"]["id"].is_null());
        assert!(value.get("ignoredField").is_none());
        assert!(value["user"].get("ignoredField").is_none());
        assert!(!out.stdout.contains("fixture-extra-nika-7k2"));
        assert!(!out.stdout.contains(ACCESS_OLD));
        assert!(!out.stdout.contains(REFRESH_OLD));
        assert!(!out.stderr.contains(ACCESS_OLD));
        assert!(!out.stderr.contains(REFRESH_OLD));
    }
}

#[test]
fn user_human_stdout_puts_name_before_id() {
    let texts = [true, false].map(|author| {
        let tmp = Tmp::new();
        let path = tmp.child("tokens.json");
        write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", FAR_EXPIRES);
        let mut body: serde_json::Value = serde_json::from_str(&user_fixture()).unwrap();
        body["user"]["flags"]["isAuthor"] = serde_json::Value::Bool(author);
        body["user"]["flags"]["isBanned"] = serde_json::Value::Bool(!author);
        let server = Server::spawn(Reply::Status {
            status: 200,
            body: body.to_string(),
        });
        let home = tmp.path().to_str().unwrap().to_string();
        let path_s = path.to_str().unwrap().to_string();
        let base = devapi_base_of(&server);
        let token_url = server.token_url();
        let out = run_bin(
            &["user"],
            &[
                ("HOME", home.as_str()),
                ("BOOSTY_TOKEN_PATH", path_s.as_str()),
                ("BOOSTY_DEVAPI_BASE", base.as_str()),
                ("BOOSTY_TOKEN_URL", token_url.as_str()),
            ],
        );
        assert_eq!(out.code, 0, "{}", out.stderr);
        assert!(out.stderr.is_empty(), "{}", out.stderr);
        assert!(!out.stderr.contains(EMAIL));
        assert!(serde_json::from_str::<serde_json::Value>(out.stdout.trim()).is_err());
        let name_at = out.stdout.find("Nika Testova").unwrap();
        let id_at = out.stdout.find("3000000001").unwrap();
        assert!(name_at < id_at, "{}", out.stdout);
        assert!(out.stdout.contains(EMAIL), "{}", out.stdout);
        assert!(!out.stdout.contains("isAuthor"), "{}", out.stdout);
        assert!(!out.stdout.contains("isBanned"), "{}", out.stdout);
        out.stdout
    });
    assert_ne!(texts[0], texts[1]);
    let mut diff = String::new();
    for (left, right) in texts[0].lines().zip(texts[1].lines()) {
        if left != right {
            diff.push_str(left);
            diff.push('\n');
            diff.push_str(right);
        }
    }
    assert!(
        diff.chars()
            .any(|ch| ('а'..='я').contains(&ch) || ('А'..='Я').contains(&ch)),
        "{diff}"
    );
    for forbidden in ["isAuthor", "isBanned", "true", "false"] {
        assert!(!diff.contains(forbidden), "{forbidden} {diff}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_debug_redacts_email() {
    let parsed: UserResponse = serde_json::from_str(&user_fixture()).unwrap();
    let debug = format!("{parsed:?}");
    assert!(!debug.contains(EMAIL), "{debug}");
    assert!(!debug.contains(ACCESS_OLD), "{debug}");
    assert!(!debug.contains(ACCESS_CANARY), "{debug}");
    let src =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/api/user.rs"))
            .unwrap();
    assert!(
        !src.split("impl")
            .skip(1)
            .any(|part| { part.split('{').next().unwrap_or(part).contains("Display") }),
        "{src}"
    );

    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_bearer(
        &path,
        ACCESS_CANARY,
        REFRESH_OLD,
        "Bearer",
        CLOCK as i64 + 61,
    );
    let token = Server::spawn(Reply::Close);
    let devapi = Server::spawn(Reply::Status {
        status: 200,
        body: format!("not-json {EMAIL_CANARY} {ACCESS_CANARY}"),
    });
    let http = http_for(&token, &devapi, Duration::from_secs(5));
    let err = crate::user::run_user(None, None, &path, &http, fixed_clock(CLOCK), rename_ok)
        .await
        .unwrap_err();
    assert_no_canary(&err, &[EMAIL_CANARY, ACCESS_CANARY]);

    let server = Server::spawn(Reply::Status {
        status: 200,
        body: user_fixture(),
    });
    let home = tmp.path().to_str().unwrap().to_string();
    let path_s = path.to_str().unwrap().to_string();
    let base = devapi_base_of(&server);
    let token_url = server.token_url();
    write_bearer(&path, ACCESS_OLD, REFRESH_OLD, "Bearer", FAR_EXPIRES);
    let out = run_bin(
        &["user"],
        &[
            ("HOME", home.as_str()),
            ("BOOSTY_TOKEN_PATH", path_s.as_str()),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", token_url.as_str()),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stderr.is_empty(), "{}", out.stderr);
    assert!(!out.stderr.contains(EMAIL));
}
