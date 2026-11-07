use std::path::Path;
use std::time::{Duration, Instant};

use super::suite::{collect_strings, json_strings};
use super::support::*;
use crate::api::subscribers::SubscribersResponse;
use crate::client;
use crate::config::StoredTokens;
use crate::error::Kind;

const KEY: &str = "subscribers-key-4k";
const CLIENT_ID: &str = "subscribers-client-4k";
const CLIENT_SECRET: &str = "subscribers-secret-4k";
const ACCESS: &str = "access-subscribers-4k";
const REFRESH: &str = "refresh-subscribers-4k";
const EMAIL: &str = "nori.kade@page-subs.invalid";
const EMAIL_TWO: &str = "mira.sol@page-subs.invalid";
const NAME: &str = "Nori Kade";
const NAME_TWO: &str = "Mira Sol";
const PAYMENTS: &str = "ZZZ4.00";
const NEXT: i64 = 9_100_000_007;
const USER_ID: i64 = 7_100_000_001;
const ORPHAN_USER: i64 = 7_100_000_002;
const FOREIGN_SUBSCRIBER: i64 = 7_300_000_099;
const FLAG_SECRET: &str = "secret-flag-4k";
const BODY_CANARY: &str = "canary-body-4k";
const UTC_STAMP: &str = "2023-11-14 22:13:20";
const MOSCOW_STAMP: &str = "2023-11-15 01:13:20";
const END_MARKERS: &[&str] = &["конец списка", "последняя страница", "больше нет"];

fn fixture_text() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join("subscribers.json"),
    )
    .unwrap()
}

fn contracts() -> (String, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    (
        std::fs::read_to_string(root.join("devapi.md")).unwrap(),
        std::fs::read_to_string(root.join("authorization_code_flow.md")).unwrap(),
    )
}

fn devapi_base_of(server: &Server) -> String {
    let base = format!("{}/v1", server.base());
    assert!(!base.ends_with('/'), "{base}");
    base
}

fn http_for(token: &Server, devapi: &Server, timeout: Duration) -> crate::client::ApiClient {
    client_for_bases(&token.token_url(), &devapi_base_of(devapi), timeout)
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

fn write_canary_tokens(path: &Path) {
    write_stored(
        path,
        &StoredTokens {
            access_token: ACCESS.to_string(),
            refresh_token: REFRESH.to_string(),
            token_type: "Bearer".to_string(),
            expires_at: 4_000_000_000,
        },
    );
}

fn assert_json_fail(stderr: &str, kind: &str, canaries: &[&str]) {
    assert_eq!(stderr.trim().lines().count(), 1, "{stderr}");
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
    assert!(!stderr.contains("Print help"), "{stderr}");
    assert!(!stderr.contains("error:"), "{stderr}");
    assert!(!stderr.contains("Usage:"), "{stderr}");
}

fn assert_russian_fail(stderr: &str, canaries: &[&str]) {
    assert!(stderr.contains('а') || stderr.contains('Н'), "{stderr}");
    assert!(serde_json::from_str::<serde_json::Value>(stderr.trim()).is_err());
    for canary in canaries {
        assert!(!stderr.contains(canary), "{canary}");
    }
}

fn pairs_of(query: &str) -> Vec<(String, String)> {
    if query.is_empty() {
        return Vec::new();
    }
    let mut pairs = form_pairs(query);
    pairs.sort();
    pairs
}

fn expect_pairs(query: &str, expected: &[(&str, &str)]) {
    let mut want: Vec<(String, String)> = expected
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect();
    want.sort();
    let got = pairs_of(query);
    assert_eq!(got, want, "{query}");
    assert!(
        got.iter()
            .all(|(key, value)| !key.is_empty() && !value.is_empty())
    );
    assert!(
        !got.iter()
            .any(|(key, value)| key == "is_free" && value.is_empty())
    );
}

fn assert_api_key_get(hit: &Hit) {
    assert_eq!(hit.method, "GET");
    assert_eq!(hit.path, "/v1/subscribers");
    assert!(!hit.path.contains('?'), "{}", hit.path);
    assert!(!hit.path.contains("blog"), "{}", hit.path);
    assert_eq!(header(hit, "x-api-key"), vec![KEY]);
    assert!(header(hit, "authorization").is_empty(), "{:?}", hit.headers);
    assert!(hit.headers.iter().all(|(name, value)| {
        name != "authorization" && !value.contains("Bearer") && !value.contains("Basic")
    }));
    assert_eq!(header(hit, "accept"), vec!["application/json"]);
    assert_eq!(
        header(hit, "user-agent"),
        vec![client::user_agent().as_str()]
    );
}

fn assert_one_json(stdout: &str) -> serde_json::Value {
    assert!(stdout.ends_with('\n'), "{stdout}");
    assert!(!stdout[..stdout.len() - 1].contains('\n'), "{stdout}");
    let value: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(value.is_object());
    assert!(value.get("kind").is_none());
    assert!(value.get("error").is_none());
    value
}

fn assert_no_end(stdout: &str) {
    assert!(!stdout.contains("следующее смещение: нет"), "{stdout}");
    for marker in END_MARKERS {
        assert!(!stdout.contains(marker), "{marker} {stdout}");
    }
}

fn cursor_line(offset: i64) -> String {
    format!("следующее смещение: {offset}")
}

fn user_body(id: i64, name: &str, email: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "name": name,
        "email": email,
        "avatarUrl": "https://cdn.page-subs.invalid/edge.png",
        "flags": {"isAuthor": true, "isBanned": true}
    })
}

fn subscription_body(id: i64, subscriber_id: i64, start: i64) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "ownerId": 7400000011_i64,
        "levelId": 7500000011_i64,
        "startTime": start,
        "endTime": start,
        "nextPayTime": start,
        "subscriberId": subscriber_id,
        "payments": PAYMENTS
    })
}

fn page_body(
    users: Vec<serde_json::Value>,
    subscriptions: Vec<serde_json::Value>,
    next_offset: Option<i64>,
) -> String {
    let mut body = serde_json::json!({
        "users": users,
        "subscriptions": subscriptions
    });
    if let Some(offset) = next_offset {
        body["extra"] = serde_json::json!({"nextOffset": offset});
    }
    body.to_string()
}

fn header_secrets(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for marker in ["X-Api-Key:", "Bearer "] {
        let mut rest = text;
        while let Some(index) = rest.find(marker) {
            let after = &rest[index + marker.len()..];
            let token = after
                .split(|ch: char| ch.is_whitespace() || ch == '"' || ch == '\'')
                .next()
                .unwrap_or("");
            if token.len() >= 16 {
                out.push(token.to_string());
            }
            let advance = marker.len() + token.len().max(1);
            rest = &rest[index + advance..];
        }
    }
    out
}

#[test]
fn subscribers_fixture_decodes_keeps_i64_zero_times_and_drops_unknown() {
    let fixture = fixture_text();
    let raw: serde_json::Value = serde_json::from_str(&fixture).unwrap();
    let parsed: SubscribersResponse = serde_json::from_str(&fixture).unwrap();
    assert_eq!(parsed.users.len(), 2);
    assert_eq!(parsed.subscriptions.len(), 2);
    let user = &parsed.users[0];
    assert_eq!(user.id, USER_ID);
    assert!(user.id > i32::MAX as i64);
    assert_eq!(user.name, NAME);
    assert_eq!(user.email, EMAIL);
    assert!(user.flags.isAuthor);
    assert!(!user.flags.isBanned);
    assert_eq!(parsed.users[1].id, ORPHAN_USER);
    assert_eq!(parsed.users[1].email, EMAIL_TWO);
    let linked = &parsed.subscriptions[0];
    assert_eq!(linked.payments, PAYMENTS);
    assert_eq!(linked.startTime, 0);
    assert_eq!(linked.endTime, 0);
    assert_eq!(linked.nextPayTime, 0);
    assert_eq!(linked.subscriberId, USER_ID);
    let foreign = &parsed.subscriptions[1];
    assert_eq!(foreign.subscriberId, FOREIGN_SUBSCRIBER);
    assert_eq!(foreign.startTime, 1_700_000_000);
    assert_eq!(foreign.endTime, 0);
    assert_eq!(foreign.nextPayTime, 0);
    assert!(parsed.users.iter().all(|row| row.id != FOREIGN_SUBSCRIBER));
    assert!(
        parsed
            .subscriptions
            .iter()
            .all(|row| row.subscriberId != ORPHAN_USER)
    );
    let cursor = parsed.extra.as_ref().unwrap().nextOffset;
    assert_eq!(cursor, NEXT);
    assert_eq!(cursor, raw["extra"]["nextOffset"].as_i64().unwrap());
    assert!(cursor > i32::MAX as i64);
    assert_ne!(cursor, 0_i64.saturating_add(5));

    let again = serde_json::to_value(&parsed).unwrap();
    let text = again.to_string();
    assert!(!text.contains("ignoredField"), "{text}");
    assert!(!text.contains("fixture-extra"), "{text}");
    assert!(again.get("ignoredField").is_none());
    assert!(again["users"][0].get("ignoredField").is_none());
    assert!(again["subscriptions"][1].get("ignoredField").is_none());
    assert!(again["extra"].get("ignoredField").is_none());
    assert_eq!(again["users"][0]["id"], USER_ID);
    assert!(again["users"][0]["id"].is_i64());
    assert!(again["users"][0]["id"].as_i64().unwrap() > i32::MAX as i64);
    assert_eq!(again["extra"]["nextOffset"], NEXT);
    assert!(again["extra"]["nextOffset"].is_i64());
    assert!(!again["extra"]["nextOffset"].is_string());
    assert!(!again["extra"]["nextOffset"].is_null());
    assert_eq!(again["subscriptions"][0]["payments"], PAYMENTS);
    assert_eq!(
        again["subscriptions"][0]["payments"].as_str().unwrap(),
        PAYMENTS
    );
    for field in ["startTime", "endTime", "nextPayTime"] {
        assert_eq!(again["subscriptions"][0][field], 0, "{field}");
        assert!(again["subscriptions"][0][field].is_i64(), "{field}");
        assert!(!again["subscriptions"][0][field].is_null(), "{field}");
        assert!(!again["subscriptions"][0][field].is_string(), "{field}");
    }
    assert!(
        again["users"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == ORPHAN_USER)
    );
    assert!(
        again["subscriptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["subscriberId"] == FOREIGN_SUBSCRIBER)
    );
}

#[test]
fn subscribers_missing_or_wrong_type_is_an_error() {
    let value: serde_json::Value = serde_json::from_str(&fixture_text()).unwrap();
    for key in ["users", "subscriptions"] {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(key);
        assert!(
            serde_json::from_value::<SubscribersResponse>(missing).is_err(),
            "{key}"
        );
        let mut null_array = value.clone();
        null_array[key] = serde_json::Value::Null;
        assert!(
            serde_json::from_value::<SubscribersResponse>(null_array).is_err(),
            "{key}"
        );
    }
    let mut id_string = value.clone();
    id_string["users"][0]["id"] = serde_json::json!("12");
    assert!(serde_json::from_value::<SubscribersResponse>(id_string).is_err());
    let mut flag_string = value.clone();
    flag_string["users"][0]["flags"]["isAuthor"] = serde_json::json!("true");
    assert!(serde_json::from_value::<SubscribersResponse>(flag_string).is_err());
    let mut payments_number = value.clone();
    payments_number["subscriptions"][0]["payments"] = serde_json::json!(4);
    assert!(serde_json::from_value::<SubscribersResponse>(payments_number).is_err());
    let mut cursor_string = value.clone();
    cursor_string["extra"]["nextOffset"] = serde_json::json!("12");
    assert!(serde_json::from_value::<SubscribersResponse>(cursor_string).is_err());
    let mut cursor_null = value.clone();
    cursor_null["extra"]["nextOffset"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<SubscribersResponse>(cursor_null).is_err());
    let mut cursor_fraction = value.clone();
    cursor_fraction["extra"]["nextOffset"] = serde_json::json!(1.5);
    assert!(serde_json::from_value::<SubscribersResponse>(cursor_fraction).is_err());

    let token = Server::spawn(Reply::Close);
    let body = format!(
        r#"{{"users":[],"subscriptions":[],"extra":{{"nextOffset":"nope"}},"note":"{EMAIL} {KEY}"}}"#
    );
    let devapi = Server::spawn(Reply::Status { status: 200, body });
    let base = devapi_base_of(&devapi);
    let token_url = token.token_url();
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_canary_tokens(&path);
    let prior = std::fs::read(&path).unwrap();
    let env = [
        ("BOOSTY_API_KEY", KEY),
        ("BOOSTY_DEVAPI_BASE", base.as_str()),
        ("BOOSTY_TOKEN_URL", token_url.as_str()),
        ("BOOSTY_CLIENT_ID", CLIENT_ID),
        ("BOOSTY_CLIENT_SECRET", CLIENT_SECRET),
        ("BOOSTY_TOKEN_PATH", path.to_str().unwrap()),
        ("HOME", tmp.path().to_str().unwrap()),
    ];
    for args in [
        vec!["subscribers", "--offset", "11", "--limit", "7"],
        vec!["--json", "subscribers", "--offset", "11", "--limit", "7"],
    ] {
        let before = devapi.hits().len();
        let out = run_bin(&args, &env);
        assert_eq!(out.code, 4, "{} {}", args.join(" "), out.stderr);
        assert!(out.stdout.is_empty(), "{}", out.stdout);
        assert_eq!(devapi.hits().len(), before + 1, "{}", args.join(" "));
        assert!(token.hits().is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), prior);
        assert!(!out.stderr.contains("18"), "{}", out.stderr);
        assert!(!out.stderr.contains("nope"), "{}", out.stderr);
        assert!(
            !out.stderr.chars().any(|ch| ch.is_ascii_digit()),
            "{}",
            out.stderr
        );
        assert!(!out.stderr.contains(EMAIL), "{}", out.stderr);
        assert!(!out.stderr.contains(KEY), "{}", out.stderr);
        if args.contains(&"--json") {
            assert_json_fail(&out.stderr, "api", &[KEY, EMAIL, "18", "nope"]);
        }
    }
}

#[test]
fn subscribers_empty_arrays_and_absent_cursor_decode() {
    let empty: SubscribersResponse =
        serde_json::from_str(r#"{"users":[],"subscriptions":[]}"#).unwrap();
    assert!(empty.users.is_empty());
    assert!(empty.subscriptions.is_empty());
    assert!(empty.extra.is_none());
    let again = serde_json::to_value(&empty).unwrap();
    assert!(again["users"].is_array());
    assert!(again["subscriptions"].is_array());
    assert!(again.get("extra").is_none());
    assert!(again.get("nextOffset").is_none());
    assert!(!again.to_string().contains("nextOffset"));

    for body in [
        r#"{"users":[],"subscriptions":[],"extra":{}}"#,
        r#"{"users":[],"subscriptions":[],"extra":{"note":1}}"#,
    ] {
        let parsed: SubscribersResponse = serde_json::from_str(body).unwrap();
        assert!(parsed.extra.is_none(), "{body}");
        let text = serde_json::to_string(&parsed).unwrap();
        assert!(!text.contains("extra"), "{text}");
        assert!(!text.contains("nextOffset"), "{text}");
    }
}

#[test]
fn subscribers_fixture_does_not_copy_contract_samples() {
    let (devapi, flow) = contracts();
    let mut secrets = header_secrets(&devapi);
    secrets.extend(header_secrets(&flow));
    assert!(!secrets.is_empty());
    let text = fixture_text();
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut fixture_strings = Vec::new();
    collect_strings(&parsed, &mut fixture_strings);
    for value in &fixture_strings {
        assert!(!devapi.contains(value), "{value}");
        assert!(!flow.contains(value), "{value}");
    }
    for contract in [&devapi, &flow] {
        for sample in json_strings(contract) {
            assert!(!text.contains(&sample), "{sample}");
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let slice = [
        "src/api/subscribers.rs",
        "src/subscribers.rs",
        "src/client.rs",
        "src/cli.rs",
        "src/main.rs",
        "src/output.rs",
        "src/api/mod.rs",
        "src/tests/subscribers.rs",
        "tests/fixtures/subscribers.json",
    ];
    for relative in slice {
        let path = root.join(relative);
        let source = std::fs::read_to_string(&path).unwrap();
        for secret in &secrets {
            assert!(
                !source.contains(secret),
                "contract sample leaked into {relative}"
            );
        }
    }
}

#[test]
fn subscribers_get_sends_one_api_key_and_exact_query() {
    let token = Server::spawn(Reply::Close);
    let devapi = Server::spawn(Reply::Status {
        status: 200,
        body: fixture_text(),
    });
    let base = devapi_base_of(&devapi);
    let token_url = token.token_url();
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_canary_tokens(&path);
    let prior = std::fs::read(&path).unwrap();
    let home = tmp.path().to_str().unwrap().to_string();
    let path_s = path.to_str().unwrap().to_string();
    let env = [
        ("BOOSTY_API_KEY", KEY),
        ("BOOSTY_DEVAPI_BASE", base.as_str()),
        ("BOOSTY_TOKEN_URL", token_url.as_str()),
        ("BOOSTY_CLIENT_ID", CLIENT_ID),
        ("BOOSTY_CLIENT_SECRET", CLIENT_SECRET),
        ("BOOSTY_TOKEN_PATH", path_s.as_str()),
        ("HOME", home.as_str()),
    ];
    struct QueryCase {
        args: &'static [&'static str],
        pairs: &'static [(&'static str, &'static str)],
    }
    let cases = [
        QueryCase {
            args: &["subscribers"],
            pairs: &[],
        },
        QueryCase {
            args: &["subscribers", "--is-free", "true"],
            pairs: &[("is_free", "true")],
        },
        QueryCase {
            args: &["--json", "subscribers", "--is-free", "true"],
            pairs: &[("is_free", "true")],
        },
        QueryCase {
            args: &["subscribers", "--is-free", "false"],
            pairs: &[("is_free", "false")],
        },
        QueryCase {
            args: &["subscribers", "--limit", "0", "--offset", "0"],
            pairs: &[("limit", "0"), ("offset", "0")],
        },
        QueryCase {
            args: &["subscribers", "--limit", "-4", "--offset", "-9"],
            pairs: &[("limit", "-4"), ("offset", "-9")],
        },
        QueryCase {
            args: &[
                "subscribers",
                "--limit",
                "4000000001",
                "--offset",
                "9100000007",
            ],
            pairs: &[("limit", "4000000001"), ("offset", "9100000007")],
        },
        QueryCase {
            args: &[
                "subscribers",
                "--is-free",
                "true",
                "--limit",
                "2",
                "--offset",
                "3",
            ],
            pairs: &[("is_free", "true"), ("limit", "2"), ("offset", "3")],
        },
    ];
    for case in &cases {
        let args = case.args;
        let expected = case.pairs;
        let before = devapi.hits().len();
        let out = run_bin(args, &env);
        assert_eq!(out.code, 0, "{} {}", args.join(" "), out.stderr);
        assert_eq!(devapi.hits().len(), before + 1, "{}", args.join(" "));
        let hit = devapi.hits().last().unwrap().clone();
        assert_api_key_get(&hit);
        expect_pairs(&hit.query, expected);
        assert!(out.stdout.contains(NAME), "{}", out.stdout);
        assert!(out.stdout.contains(NAME_TWO), "{}", out.stdout);
        assert!(out.stdout.contains(EMAIL), "{}", out.stdout);
        assert!(!out.stdout.contains(KEY), "{}", out.stdout);
        assert!(!out.stderr.contains(KEY), "{}", out.stderr);
        assert_eq!(std::fs::read(&path).unwrap(), prior);
        assert!(token.hits().is_empty(), "{}", args.join(" "));
    }
    assert_eq!(devapi.hits().len(), cases.len());
}

#[test]
fn subscribers_omits_is_free_when_flag_absent() {
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: fixture_text(),
    });
    let base = devapi_base_of(&server);
    let token_url = server.token_url();
    let env = [
        ("BOOSTY_API_KEY", KEY),
        ("BOOSTY_DEVAPI_BASE", base.as_str()),
        ("BOOSTY_TOKEN_URL", token_url.as_str()),
    ];
    let out = run_bin(&["subscribers"], &env);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(server.hits().len(), 1);
    expect_pairs(&server.hits()[0].query, &[]);
    assert!(
        !pairs_of(&server.hits()[0].query)
            .iter()
            .any(|(key, _)| key == "is_free")
    );

    let limited = run_bin(&["subscribers", "--limit", "5", "--offset", "2"], &env);
    assert_eq!(limited.code, 0, "{}", limited.stderr);
    assert_eq!(server.hits().len(), 2);
    let query = &server.hits()[1].query;
    expect_pairs(query, &[("limit", "5"), ("offset", "2")]);
    assert!(
        !pairs_of(query)
            .iter()
            .any(|(key, value)| key == "is_free" || value.is_empty())
    );
}

#[test]
fn missing_or_empty_api_key_is_auth_and_offline() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_canary_tokens(&path);
    let prior = std::fs::read(&path).unwrap();
    let canaries = [KEY, ACCESS, REFRESH, CLIENT_SECRET, EMAIL];
    for key in [None, Some("")] {
        let server = Server::spawn(Reply::Close);
        let base = devapi_base_of(&server);
        let token_url = server.token_url();
        let mut env = vec![
            ("HOME", tmp.path().to_str().unwrap()),
            ("BOOSTY_TOKEN_PATH", path.to_str().unwrap()),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", token_url.as_str()),
            ("BOOSTY_CLIENT_ID", CLIENT_ID),
            ("BOOSTY_CLIENT_SECRET", CLIENT_SECRET),
        ];
        if let Some(value) = key {
            env.push(("BOOSTY_API_KEY", value));
        }
        for args in [
            vec!["subscribers"],
            vec!["--json", "subscribers"],
            vec!["subscribers", "--json"],
        ] {
            let out = run_bin(&args, &env);
            assert_eq!(out.code, 3, "{:?} {}", args, out.stderr);
            assert!(out.stdout.is_empty());
            assert!(server.hits().is_empty(), "{:?}", args);
            assert_eq!(std::fs::read(&path).unwrap(), prior);
            assert!(!out.stderr.contains("Bearer"));
            assert!(!out.stdout.contains("Bearer"));
            if args.contains(&"--json") {
                assert_json_fail(&out.stderr, "auth", &canaries);
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(out.stderr.trim()).unwrap()["error"],
                    "missing API key"
                );
            } else {
                assert_russian_fail(&out.stderr, &canaries);
                assert!(out.stderr.contains("Нет ключа API."));
            }
        }
    }
}

#[test]
fn key_present_ignores_missing_home_and_token_path() {
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: fixture_text(),
    });
    let base = devapi_base_of(&server);
    let out = run_bin(
        &["subscribers"],
        &[
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", &server.token_url()),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(server.hits().len(), 1);
    assert_eq!(server.hits()[0].path, "/v1/subscribers");

    let tmp = Tmp::new();
    let missing = tmp.child("absent-tokens.json");
    assert!(!missing.exists());
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: fixture_text(),
    });
    let base = devapi_base_of(&server);
    let out = run_bin(
        &["subscribers"],
        &[
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", &server.token_url()),
            ("BOOSTY_TOKEN_PATH", missing.to_str().unwrap()),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(server.hits().len(), 1);
    assert!(!missing.exists());
}

#[test]
fn json_stdout_preserves_next_offset() {
    let fixture = fixture_text();
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: fixture.clone(),
    });
    let base = devapi_base_of(&server);
    let token_url = server.token_url();
    let env = [
        ("BOOSTY_API_KEY", KEY),
        ("BOOSTY_DEVAPI_BASE", base.as_str()),
        ("BOOSTY_TOKEN_URL", token_url.as_str()),
    ];
    for args in [
        vec!["--json", "subscribers", "--limit", "5", "--offset", "2"],
        vec!["subscribers", "--json", "--limit", "5", "--offset", "2"],
    ] {
        let before = server.hits().len();
        let out = run_bin(&args, &env);
        assert_eq!(out.code, 0, "{} {}", args.join(" "), out.stderr);
        assert!(out.stderr.is_empty(), "{}", out.stderr);
        assert_eq!(server.hits().len(), before + 1);
        let value = assert_one_json(&out.stdout);
        assert!(!out.stdout.contains("ignoredField"));
        assert!(!out.stdout.contains("fixture-extra"));
        assert!(!out.stdout.contains(KEY));
        assert!(!out.stderr.contains(KEY));
        assert_eq!(value["users"][0]["email"], EMAIL);
        assert_eq!(value["subscriptions"][0]["payments"], PAYMENTS);
        assert_eq!(
            value["subscriptions"][0]["payments"].as_str().unwrap(),
            PAYMENTS
        );
        assert_eq!(value["subscriptions"][0]["startTime"], 0);
        assert_eq!(value["subscriptions"][0]["endTime"], 0);
        assert_eq!(value["subscriptions"][0]["nextPayTime"], 0);
        assert!(value["subscriptions"][0]["startTime"].is_i64());
        assert!(!value["subscriptions"][0]["startTime"].is_null());
        assert_eq!(value["extra"]["nextOffset"], NEXT);
        assert!(value["extra"]["nextOffset"].is_i64());
        assert!(!value["extra"]["nextOffset"].is_string());
        assert!(!value["extra"]["nextOffset"].is_null());
        assert_ne!(value["extra"]["nextOffset"], 7);
        let user_ids: Vec<_> = value["users"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].as_i64().unwrap())
            .collect();
        let subscriber_ids: Vec<_> = value["subscriptions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["subscriberId"].as_i64().unwrap())
            .collect();
        assert!(user_ids.contains(&ORPHAN_USER));
        assert!(!subscriber_ids.contains(&ORPHAN_USER));
        assert!(subscriber_ids.contains(&FOREIGN_SUBSCRIBER));
        assert!(!user_ids.contains(&FOREIGN_SUBSCRIBER));
    }

    let same = Server::spawn(Reply::Status {
        status: 200,
        body: fixture,
    });
    let base = devapi_base_of(&same);
    let out = run_bin(
        &[
            "--json",
            "subscribers",
            "--offset",
            "9100000007",
            "--limit",
            "3",
        ],
        &[
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", &same.token_url()),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(same.hits().len(), 1);
    let value = assert_one_json(&out.stdout);
    assert_eq!(value["extra"]["nextOffset"], NEXT);
    assert_ne!(value["extra"]["nextOffset"], NEXT + 3);

    let short = page_body(vec![user_body(USER_ID, NAME, EMAIL)], vec![], Some(42));
    let short_server = Server::spawn(Reply::Status {
        status: 200,
        body: short,
    });
    let base = devapi_base_of(&short_server);
    let out = run_bin(
        &["subscribers", "--json", "--limit", "9", "--offset", "1"],
        &[
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", &short_server.token_url()),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(short_server.hits().len(), 1);
    let value = assert_one_json(&out.stdout);
    assert_eq!(value["users"].as_array().unwrap().len(), 1);
    assert_eq!(value["extra"]["nextOffset"], 42);
    assert_ne!(value["extra"]["nextOffset"], 10);

    let equal = page_body(vec![user_body(USER_ID, NAME, EMAIL)], vec![], Some(42));
    let equal_server = Server::spawn(Reply::Status {
        status: 200,
        body: equal,
    });
    let base = devapi_base_of(&equal_server);
    let out = run_bin(
        &["--json", "subscribers", "--offset", "42", "--limit", "5"],
        &[
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", &equal_server.token_url()),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(equal_server.hits().len(), 1);
    let value = assert_one_json(&out.stdout);
    assert_eq!(value["extra"]["nextOffset"], 42);
    assert_ne!(value["extra"]["nextOffset"], 47);

    let absent = page_body(vec![user_body(USER_ID, NAME, EMAIL)], vec![], None);
    let absent_server = Server::spawn(Reply::Status {
        status: 200,
        body: absent,
    });
    let base = devapi_base_of(&absent_server);
    let out = run_bin(
        &["--json", "subscribers"],
        &[
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", &absent_server.token_url()),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    let value = assert_one_json(&out.stdout);
    assert!(value.get("extra").is_none(), "{value}");
    assert!(value.get("nextOffset").is_none(), "{value}");
    assert!(!out.stdout.contains("nextOffset"), "{}", out.stdout);

    let empty = page_body(vec![], vec![], None);
    let empty_server = Server::spawn(Reply::Status {
        status: 200,
        body: empty,
    });
    let base = devapi_base_of(&empty_server);
    let out = run_bin(
        &["--json", "subscribers"],
        &[
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", &empty_server.token_url()),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    let value = assert_one_json(&out.stdout);
    assert!(value["users"].is_array());
    assert!(value["users"].as_array().unwrap().is_empty());
    assert!(value["subscriptions"].is_array());
    assert!(value["subscriptions"].as_array().unwrap().is_empty());
    assert!(value.get("extra").is_none());
    assert!(value.get("nextOffset").is_none());
}

#[test]
fn human_stdout_prints_cursor_name_email_and_times() {
    let fixture = fixture_text();
    for (tz, stamp, other) in [
        ("UTC", UTC_STAMP, MOSCOW_STAMP),
        ("Europe/Moscow", MOSCOW_STAMP, UTC_STAMP),
    ] {
        let server = Server::spawn(Reply::Status {
            status: 200,
            body: fixture.clone(),
        });
        let base = devapi_base_of(&server);
        let out = run_bin(
            &["subscribers"],
            &[
                ("BOOSTY_API_KEY", KEY),
                ("BOOSTY_DEVAPI_BASE", base.as_str()),
                ("BOOSTY_TOKEN_URL", &server.token_url()),
                ("TZ", tz),
            ],
        );
        assert_eq!(out.code, 0, "{}", out.stderr);
        assert!(out.stderr.is_empty(), "{}", out.stderr);
        assert!(serde_json::from_str::<serde_json::Value>(out.stdout.trim()).is_err());
        assert!(out.stdout.contains(stamp), "{tz} {}", out.stdout);
        assert!(!out.stdout.contains(other), "{tz} {}", out.stdout);
        assert!(!out.stdout.contains("1970-01-01"), "{}", out.stdout);
        assert!(
            out.stdout.contains('а') || out.stdout.contains('П'),
            "{}",
            out.stdout
        );
        assert!(out.stdout.contains("Подписчики"), "{}", out.stdout);
        assert!(out.stdout.contains("Подписки"), "{}", out.stdout);
        assert!(
            out.stdout
                .lines()
                .any(|line| line == cursor_line(NEXT).as_str())
        );
        assert!(!out.stdout.contains("9,100,000,007"), "{}", out.stdout);
        assert!(!out.stdout.contains("9 100 000 007"), "{}", out.stdout);
        let name_at = out.stdout.find(NAME).unwrap();
        let id_at = out.stdout.find(&USER_ID.to_string()).unwrap();
        assert!(name_at < id_at, "{}", out.stdout);
        let email_at = out.stdout.find(EMAIL).unwrap();
        let second_at = out.stdout.find(NAME_TWO).unwrap();
        assert!(email_at < second_at, "{}", out.stdout);
        assert!(out.stdout.contains(NAME_TWO), "{}", out.stdout);
        assert!(out.stdout.contains(&FOREIGN_SUBSCRIBER.to_string()));
        assert!(out.stdout.contains(EMAIL));
        assert!(out.stdout.contains(EMAIL_TWO));
        for key in ["isAuthor", "isBanned", "true", "false"] {
            assert!(!out.stdout.contains(key), "{key} {}", out.stdout);
        }
        assert!(out.stdout.contains("нет"), "{}", out.stdout);
        assert!(
            out.stdout.contains(&format!("платежи: {PAYMENTS}")),
            "{}",
            out.stdout
        );
        assert!(!out.stdout.contains("4.00 ZZZ"), "{}", out.stdout);
        assert!(!out.stdout.contains(KEY));
        assert!(!out.stderr.contains(EMAIL));
        assert_eq!(server.hits().len(), 1);
    }

    let zero_cursor = page_body(
        vec![user_body(USER_ID, NAME, EMAIL)],
        vec![subscription_body(7_200_000_001, USER_ID, 1_700_000_000)],
        Some(0),
    );
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: zero_cursor,
    });
    let base = devapi_base_of(&server);
    let out = run_bin(
        &["subscribers"],
        &[
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", &server.token_url()),
            ("TZ", "UTC"),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        out.stdout
            .lines()
            .any(|line| line == "следующее смещение: 0")
    );
    assert!(
        !out.stdout.contains("следующее смещение: нет"),
        "{}",
        out.stdout
    );
    assert!(!out.stdout.contains("нет"), "{}", out.stdout);
    assert!(out.stdout.contains(UTC_STAMP), "{}", out.stdout);
    assert!(out.stdout.contains(&format!("платежи: {PAYMENTS}")));
    assert!(!out.stdout.contains("4.00 ZZZ"));
    assert_eq!(server.hits().len(), 1);

    let same = Server::spawn(Reply::Status {
        status: 200,
        body: fixture_text(),
    });
    let base = devapi_base_of(&same);
    let out = run_bin(
        &["subscribers", "--offset", "9100000007"],
        &[
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", &same.token_url()),
            ("TZ", "UTC"),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        out.stdout
            .lines()
            .any(|line| line == cursor_line(NEXT).as_str())
    );
    assert_no_end(&out.stdout);
    assert!(!out.stdout.is_empty());
    assert_eq!(same.hits().len(), 1);
}

#[test]
fn missing_cursor_short_page_and_same_offset_are_not_end() {
    let missing = page_body(
        vec![user_body(USER_ID, NAME, EMAIL)],
        vec![subscription_body(7_200_000_001, USER_ID, 0)],
        None,
    );
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: missing,
    });
    let base = devapi_base_of(&server);
    let out = run_bin(
        &["subscribers"],
        &[
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", &server.token_url()),
            ("TZ", "UTC"),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(!out.stdout.is_empty());
    assert!(
        out.stdout.contains('а') || out.stdout.contains('П'),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("нет"), "{}", out.stdout);
    assert!(!out.stdout.contains("следующее смещение"), "{}", out.stdout);
    assert_no_end(&out.stdout);
    assert!(!out.stdout.contains("1970-01-01"));
    assert_eq!(server.hits().len(), 1);

    let short = page_body(vec![user_body(USER_ID, NAME, EMAIL)], vec![], Some(42));
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: short,
    });
    let base = devapi_base_of(&server);
    let out = run_bin(
        &["subscribers", "--limit", "5", "--offset", "1"],
        &[
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", &server.token_url()),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(!out.stdout.is_empty());
    assert!(out.stdout.contains(NAME));
    assert!(
        out.stdout
            .lines()
            .any(|line| line == "следующее смещение: 42")
    );
    assert_no_end(&out.stdout);
    assert_eq!(server.hits().len(), 1);

    let same = page_body(vec![user_body(USER_ID, NAME, EMAIL)], vec![], Some(8));
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: same,
    });
    let base = devapi_base_of(&server);
    let out = run_bin(
        &["subscribers", "--offset", "8", "--limit", "5"],
        &[
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", &server.token_url()),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(!out.stdout.is_empty());
    assert!(
        out.stdout
            .lines()
            .any(|line| line == "следующее смещение: 8")
    );
    assert_no_end(&out.stdout);
    assert_eq!(server.hits().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn email_stays_out_of_stderr_debug_and_logs() {
    let parsed: SubscribersResponse = serde_json::from_str(&fixture_text()).unwrap();
    let debug = format!("{parsed:?}");
    assert!(!debug.contains(EMAIL), "{debug}");
    assert!(!debug.contains(EMAIL_TWO), "{debug}");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative in ["src/api/subscribers.rs", "src/subscribers.rs"] {
        let src = std::fs::read_to_string(root.join(relative)).unwrap();
        assert!(
            !src.split("impl").skip(1).any(|part| part
                .split('{')
                .next()
                .unwrap_or(part)
                .contains("Display")),
            "{relative}\n{src}"
        );
    }

    let server = Server::spawn(Reply::Status {
        status: 200,
        body: fixture_text(),
    });
    let base = devapi_base_of(&server);
    let token_url = server.token_url();
    let env = [
        ("BOOSTY_API_KEY", KEY),
        ("BOOSTY_DEVAPI_BASE", base.as_str()),
        ("BOOSTY_TOKEN_URL", token_url.as_str()),
    ];
    let human = run_bin(&["subscribers"], &env);
    assert_eq!(human.code, 0, "{}", human.stderr);
    assert!(human.stdout.contains(EMAIL));
    assert!(human.stderr.is_empty(), "{}", human.stderr);
    assert!(!human.stderr.contains(EMAIL));
    let json = run_bin(&["--json", "subscribers"], &env);
    assert_eq!(json.code, 0, "{}", json.stderr);
    assert!(json.stdout.contains(EMAIL));
    assert!(json.stderr.is_empty(), "{}", json.stderr);
    assert!(!json.stderr.contains(EMAIL));

    let token = Server::spawn(Reply::Close);
    let err_body = format!(r#"{{"email":"{EMAIL}","key":"{KEY}","note":"{BODY_CANARY}"}}"#);
    let devapi = Server::spawn(Reply::Status {
        status: 401,
        body: err_body,
    });
    let http = http_for(&token, &devapi, Duration::from_secs(5));
    let err = crate::subscribers::run_subscribers(Some(KEY), &http, None, None, None)
        .await
        .unwrap_err();
    let debug = format!("{err:?}");
    for canary in [EMAIL, KEY, BODY_CANARY] {
        assert!(!debug.contains(canary), "{canary} {debug}");
        assert!(!err.english().contains(canary), "{canary}");
        assert!(!err.russian().contains(canary), "{canary}");
    }
    let failed = Server::spawn(Reply::Status {
        status: 500,
        body: format!(r#"{{"email":"{EMAIL}","key":"{KEY}"}}"#),
    });
    let base = devapi_base_of(&failed);
    let out = run_bin(
        &["subscribers"],
        &[
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", &failed.token_url()),
        ],
    );
    assert_eq!(out.code, 4, "{}", out.stderr);
    assert!(out.stdout.is_empty());
    assert!(!out.stderr.contains(EMAIL));
    assert!(!out.stderr.contains(KEY));
    assert!(!out.stderr.contains(BODY_CANARY));
}

#[test]
fn devapi_failure_is_api_without_bearer_or_second_request() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_canary_tokens(&path);
    let prior = std::fs::read(&path).unwrap();
    let token = Server::spawn(Reply::Close);
    let token_url = token.token_url();
    let body = format!(r#"{{"email":"{EMAIL}","key":"{KEY}","note":"{BODY_CANARY}"}}"#);
    let canaries = [KEY, EMAIL, BODY_CANARY, ACCESS, REFRESH, CLIENT_SECRET];
    for status in [401_u16, 403] {
        let devapi = Server::spawn(Reply::Status {
            status,
            body: body.clone(),
        });
        let base = devapi_base_of(&devapi);
        let env = [
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", token_url.as_str()),
            ("BOOSTY_CLIENT_ID", CLIENT_ID),
            ("BOOSTY_CLIENT_SECRET", CLIENT_SECRET),
            ("BOOSTY_TOKEN_PATH", path.to_str().unwrap()),
            ("HOME", tmp.path().to_str().unwrap()),
        ];
        for args in [
            vec!["subscribers"],
            vec!["--json", "subscribers"],
            vec!["subscribers", "--json"],
        ] {
            let before = devapi.hits().len();
            let out = run_bin(&args, &env);
            assert_eq!(out.code, 4, "{status} {:?} {}", args, out.stderr);
            assert!(out.stdout.is_empty());
            assert_eq!(devapi.hits().len(), before + 1);
            let hits = devapi.hits();
            let hit = hits.last().unwrap();
            assert!(header(hit, "authorization").is_empty(), "{:?}", hit.headers);
            assert!(
                hit.headers
                    .iter()
                    .all(|(_, value)| !value.contains("Bearer") && !value.contains("Basic"))
            );
            assert!(token.hits().is_empty());
            assert_eq!(std::fs::read(&path).unwrap(), prior);
            if args.contains(&"--json") {
                assert_json_fail(&out.stderr, "api", &canaries);
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(out.stderr.trim()).unwrap()["error"],
                    "DevAPI request failed"
                );
            } else {
                assert_russian_fail(&out.stderr, &canaries);
            }
            for canary in &canaries {
                assert!(!out.stderr.contains(canary), "{canary}");
                assert!(!out.stdout.contains(canary), "{canary}");
            }
        }
    }

    let cases = [
        ("not-json", format!("not-json {EMAIL} {KEY} {BODY_CANARY}")),
        ("missing-users", r#"{"subscriptions":[]}"#.to_string()),
        ("missing-subs", r#"{"users":[]}"#.to_string()),
        (
            "null-users",
            r#"{"users":null,"subscriptions":[]}"#.to_string(),
        ),
    ];
    for (name, body) in cases {
        let server = Server::spawn(Reply::Status { status: 200, body });
        let base = devapi_base_of(&server);
        let out = run_bin(
            &["--json", "subscribers"],
            &[
                ("BOOSTY_API_KEY", KEY),
                ("BOOSTY_DEVAPI_BASE", base.as_str()),
                ("BOOSTY_TOKEN_URL", &server.token_url()),
            ],
        );
        assert_eq!(out.code, 4, "{name} {}", out.stderr);
        assert!(out.stdout.is_empty());
        assert!(server.hits().len() <= 1, "{name}");
        assert!(header(&server.hits()[0], "authorization").is_empty());
        assert_json_fail(&out.stderr, "api", &[KEY, EMAIL, BODY_CANARY]);
    }
    let closed = Server::spawn(Reply::Close);
    let base = devapi_base_of(&closed);
    let out = run_bin(
        &["--json", "subscribers"],
        &[
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", &closed.token_url()),
        ],
    );
    assert_eq!(out.code, 4, "{}", out.stderr);
    assert!(out.stdout.is_empty());
    assert!(closed.hits().len() <= 1);
    assert_json_fail(&out.stderr, "api", &[KEY, EMAIL]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subscribers_timeout_uses_existing_client() {
    let client_src =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/client.rs"))
            .unwrap();
    let main_src =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs")).unwrap();
    assert_eq!(client_src.matches(".timeout(").count(), 1);
    assert!(main_src.contains("HTTP_TIMEOUT"));
    assert_eq!(client::HTTP_TIMEOUT, Duration::from_secs(30));
    let token = Server::spawn(Reply::Close);
    let devapi = Server::spawn(Reply::Silent);
    let http = http_for(&token, &devapi, Duration::from_millis(200));
    let started = Instant::now();
    let err = crate::subscribers::run_subscribers(Some(KEY), &http, None, None, None)
        .await
        .unwrap_err();
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_kind(&err, Kind::Api);
    assert!(devapi.hits().len() <= 1);
    assert!(token.hits().is_empty());
    assert!(!err.english().contains(KEY));
    assert!(!format!("{err:?}").contains(KEY));
}

#[test]
fn parses_json_without_content_type() {
    let server = Server::spawn(Reply::NoContentType {
        status: 200,
        body: fixture_text(),
    });
    let base = devapi_base_of(&server);
    let out = run_bin(
        &["--json", "subscribers"],
        &[
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", &server.token_url()),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stderr.is_empty(), "{}", out.stderr);
    let value = assert_one_json(&out.stdout);
    let expected: SubscribersResponse = serde_json::from_str(&fixture_text()).unwrap();
    assert_eq!(value, serde_json::to_value(&expected).unwrap());
    assert_eq!(server.hits().len(), 1);
}

#[test]
fn bad_devapi_base_is_io_without_request() {
    let server = Server::spawn(Reply::Close);
    let token_url = server.token_url();
    for args in [
        vec!["--json", "subscribers"],
        vec!["subscribers", "--json"],
        vec!["subscribers"],
    ] {
        let out = run_bin(
            &args,
            &[
                ("BOOSTY_API_KEY", KEY),
                ("BOOSTY_DEVAPI_BASE", "not a url"),
                ("BOOSTY_TOKEN_URL", token_url.as_str()),
            ],
        );
        assert_eq!(out.code, 5, "{:?} {}", args, out.stderr);
        assert!(out.stdout.is_empty());
        assert!(server.hits().is_empty());
        if args.contains(&"--json") {
            assert_json_fail(&out.stderr, "io", &[KEY]);
        }
    }
}

#[test]
fn usage_rejects_extra_flags_without_network() {
    let server = Server::spawn(Reply::Close);
    let base = devapi_base_of(&server);
    let token_url = server.token_url();
    let env = [
        ("BOOSTY_API_KEY", KEY),
        ("BOOSTY_DEVAPI_BASE", base.as_str()),
        ("BOOSTY_TOKEN_URL", token_url.as_str()),
    ];
    let tails: &[&[&str]] = &[
        &["extra"],
        &["--blog", FLAG_SECRET],
        &["--blog-id", FLAG_SECRET],
        &["--is-deleted"],
        &["--is-deleted", FLAG_SECRET],
        &["--api-key", FLAG_SECRET],
        &["--is-free"],
        &["--is-free", "TRUE"],
        &["--is-free", "1"],
        &["--is-free", "0"],
        &["--is-free", "yes"],
        &["--limit", "1.5"],
        &["--limit", "abc"],
        &["--offset", "2.0"],
        &["--offset", "nope"],
        &["--limit"],
        &["--offset"],
    ];
    let canaries = [FLAG_SECRET, KEY, CLIENT_SECRET];
    for tail in tails {
        let mut plain = vec!["subscribers"];
        plain.extend_from_slice(tail);
        let human = run_bin(&plain, &env);
        assert_eq!(human.code, 2, "{} {}", plain.join(" "), human.stderr);
        assert!(human.stdout.is_empty());
        assert_russian_fail(&human.stderr, &canaries);
        assert!(!human.stderr.contains("Print help"));
        assert!(!human.stderr.contains("Usage:"));
        assert!(server.hits().is_empty(), "{}", plain.join(" "));

        let mut before = vec!["--json", "subscribers"];
        before.extend_from_slice(tail);
        let mut after = vec!["subscribers"];
        after.extend_from_slice(tail);
        after.push("--json");
        for args in [before, after] {
            let out = run_bin(&args, &env);
            assert_eq!(out.code, 2, "{} {}", args.join(" "), out.stderr);
            assert!(out.stdout.is_empty());
            assert_json_fail(&out.stderr, "usage", &canaries);
            assert!(server.hits().is_empty(), "{}", args.join(" "));
        }
    }
}

#[test]
fn empty_page_is_success() {
    let body = page_body(vec![], vec![], None);
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: body.clone(),
    });
    let base = devapi_base_of(&server);
    let env = [
        ("BOOSTY_API_KEY", KEY),
        ("BOOSTY_DEVAPI_BASE", base.as_str()),
        ("BOOSTY_TOKEN_URL", &server.token_url()),
    ];
    let human = run_bin(&["subscribers"], &env);
    assert_eq!(human.code, 0, "{}", human.stderr);
    assert!(human.stderr.is_empty(), "{}", human.stderr);
    assert!(!human.stdout.is_empty());
    assert!(
        human.stdout.contains('а') || human.stdout.contains('П'),
        "{}",
        human.stdout
    );
    assert!(human.stdout.contains("Подписчики"));
    assert!(human.stdout.contains("Подписки"));
    assert!(!human.stdout.contains("следующее смещение"));
    assert_no_end(&human.stdout);
    assert!(serde_json::from_str::<serde_json::Value>(human.stdout.trim()).is_err());

    let json = run_bin(&["--json", "subscribers"], &env);
    assert_eq!(json.code, 0, "{}", json.stderr);
    let value = assert_one_json(&json.stdout);
    assert!(value["users"].is_array());
    assert!(value["users"].as_array().unwrap().is_empty());
    assert!(value["subscriptions"].as_array().unwrap().is_empty());
    assert!(value.get("kind").is_none());
    assert!(value.get("error").is_none());
    assert!(value.get("extra").is_none());
    assert!(value.get("nextOffset").is_none());
    assert!(!json.stdout.contains("nextOffset"));
}

#[test]
fn help_is_russian() {
    let root = run_bin(&["--help"], &[]);
    let page = run_bin(&["subscribers", "--help"], &[]);
    for item in [&root, &page] {
        assert_eq!(item.code, 0, "{}", item.stderr);
        assert!(item.stdout.contains("Использование"), "{}", item.stdout);
        assert!(!item.stdout.contains("Print help"), "{}", item.stdout);
        assert!(!item.stdout.contains("Usage:"), "{}", item.stdout);
        assert!(!item.stderr.contains("Print help"));
        assert!(
            !item
                .stdout
                .contains("Print this message or the help of the given subcommand(s)"),
            "{}",
            item.stdout
        );
    }
    assert!(
        root.stdout.contains("Показать подписчиков"),
        "{}",
        root.stdout
    );
    assert!(
        page.stdout.contains("Показать подписчиков"),
        "{}",
        page.stdout
    );
    for flag in ["--is-free", "--limit", "--offset"] {
        assert!(page.stdout.contains(flag), "{flag}\n{}", page.stdout);
    }
    assert!(
        page.stdout
            .contains("Только бесплатные при true, только платные при false")
    );
    assert!(page.stdout.contains("Лимит записей"));
    assert!(page.stdout.contains("Смещение следующей страницы"));
    assert!(!page.stdout.contains("possible values"), "{}", page.stdout);
    assert!(!page.stdout.contains("Possible values"), "{}", page.stdout);
}
