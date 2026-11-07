use std::path::Path;
use std::time::{Duration, Instant};

use super::suite::{collect_strings, json_strings};
use super::support::*;
use crate::api::levels::{SubscriptionLevelsResponse, SubscriptionsResponse};
use crate::client;
use crate::config::StoredTokens;
use crate::error::Kind;

const KEY: &str = "levels-key-9q";
const CLIENT_ID: &str = "levels-client-9q";
const CLIENT_SECRET: &str = "levels-secret-9q";
const ACCESS: &str = "access-levels-9q";
const REFRESH: &str = "refresh-levels-9q";
const BODY_CANARY: &str = "canary-body-9q";
const KEY_CANARY: &str = "canary-key-9q";
const FLAG_SECRET: &str = "secret-flag-9q";
const COMMANDS: &[&str] = &["subscription-levels", "subscriptions"];
const UTC_STAMP: &str = "2023-11-14 22:13:20";
const MOSCOW_STAMP: &str = "2023-11-15 01:13:20";

fn levels_fixture() -> String {
    fixture("subscription_levels.json")
}

fn subscriptions_fixture() -> String {
    fixture("subscriptions.json")
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
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
}

fn assert_russian_fail(stderr: &str, canaries: &[&str]) {
    assert!(stderr.contains('а') || stderr.contains('Н'), "{stderr}");
    assert!(serde_json::from_str::<serde_json::Value>(stderr.trim()).is_err());
    for canary in canaries {
        assert!(!stderr.contains(canary), "{canary}");
    }
}

fn assert_api_key_get(hit: &Hit, path: &str) {
    assert_eq!(hit.method, "GET");
    assert_eq!(hit.path, path);
    assert!(hit.query.is_empty(), "{}", hit.query);
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

fn command_path(command: &str) -> &'static str {
    match command {
        "subscription-levels" => "/v1/subscription_levels",
        "subscriptions" => "/v1/subscriptions",
        other => panic!("unknown command {other}"),
    }
}

fn command_fixture(command: &str) -> String {
    match command {
        "subscription-levels" => levels_fixture(),
        "subscriptions" => subscriptions_fixture(),
        other => panic!("unknown command {other}"),
    }
}

fn named_block(stdout: &str, name: &str) -> String {
    let start = stdout
        .find(name)
        .unwrap_or_else(|| panic!("{name} missing in {stdout}"));
    let rest = &stdout[start..];
    let end = rest.find("\n\n").unwrap_or(rest.len());
    rest[..end].trim_end().to_string()
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
fn levels_fixture_decodes_keeps_deleted_i64_and_string_price() {
    let parsed: SubscriptionLevelsResponse = serde_json::from_str(&levels_fixture()).unwrap();
    assert!(
        parsed
            .subscriptionLevels
            .iter()
            .any(|level| level.isDeleted)
    );
    assert_eq!(parsed.subscriptionLevels.len(), 3);
    let wide = parsed
        .subscriptionLevels
        .iter()
        .find(|level| level.id == 4_000_000_001)
        .unwrap();
    assert!(wide.id > i32::MAX as i64);
    assert_eq!(wide.price, "AAA1.20");
    let again = serde_json::to_value(&parsed).unwrap();
    let text = again.to_string();
    assert!(!text.contains("ignoredField"), "{text}");
    assert!(!text.contains("fixture-extra-polka-9q"), "{text}");
    assert!(again.get("subscriptionLevels").is_some());
    assert!(!again["subscriptionLevels"].is_null());
    for level in again["subscriptionLevels"].as_array().unwrap() {
        for key in ["id", "price", "authorId", "name", "createTime", "isDeleted"] {
            assert!(level.get(key).is_some(), "{key}");
            assert!(!level[key].is_null(), "{key}");
        }
        assert!(level["id"].is_i64());
        assert!(level["price"].is_string());
        assert!(level["isDeleted"].is_boolean());
    }
    assert_eq!(again["subscriptionLevels"][0]["id"], 4_000_000_001_i64);
    assert_eq!(again["subscriptionLevels"][0]["price"], "AAA1.20");
}

#[test]
fn levels_fixture_missing_or_wrong_type_is_an_error() {
    let value: serde_json::Value = serde_json::from_str(&levels_fixture()).unwrap();
    let mut missing_array = value.clone();
    missing_array
        .as_object_mut()
        .unwrap()
        .remove("subscriptionLevels");
    assert!(serde_json::from_value::<SubscriptionLevelsResponse>(missing_array).is_err());
    for field in ["id", "price", "authorId", "name", "createTime", "isDeleted"] {
        let mut broken = value.clone();
        broken["subscriptionLevels"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            serde_json::from_value::<SubscriptionLevelsResponse>(broken).is_err(),
            "{field}"
        );
    }
    let mut price_number = value.clone();
    price_number["subscriptionLevels"][0]["price"] = serde_json::json!(11);
    assert!(serde_json::from_value::<SubscriptionLevelsResponse>(price_number).is_err());
    let mut deleted_string = value.clone();
    deleted_string["subscriptionLevels"][0]["isDeleted"] = serde_json::json!("true");
    assert!(serde_json::from_value::<SubscriptionLevelsResponse>(deleted_string).is_err());
    let mut id_string = value.clone();
    id_string["subscriptionLevels"][0]["id"] = serde_json::json!("4000000001");
    assert!(serde_json::from_value::<SubscriptionLevelsResponse>(id_string).is_err());
    let mut null_array = value.clone();
    null_array["subscriptionLevels"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<SubscriptionLevelsResponse>(null_array).is_err());
}

#[test]
fn levels_empty_array_decodes() {
    let parsed: SubscriptionLevelsResponse =
        serde_json::from_str(r#"{"subscriptionLevels":[]}"#).unwrap();
    assert!(parsed.subscriptionLevels.is_empty());
}

#[test]
fn subscriptions_fixture_decodes_zero_times_and_keeps_unmatched_rows() {
    let parsed: SubscriptionsResponse = serde_json::from_str(&subscriptions_fixture()).unwrap();
    assert!(!parsed.subscriptions.is_empty());
    assert!(!parsed.subscriptionLevels.is_empty());
    assert!(
        parsed
            .subscriptionLevels
            .iter()
            .any(|level| level.isDeleted)
    );
    let zero = parsed
        .subscriptions
        .iter()
        .find(|row| row.endTime == 0 && row.nextPayTime == 0)
        .unwrap();
    assert_eq!(zero.endTime, 0);
    assert_eq!(zero.nextPayTime, 0);
    assert_eq!(zero.payments, "ZZZ4.00");
    assert!(
        parsed
            .subscriptions
            .iter()
            .any(|row| row.id > i32::MAX as i64)
            || parsed
                .subscriptionLevels
                .iter()
                .any(|level| level.id > i32::MAX as i64)
    );
    let level_ids: Vec<i64> = parsed
        .subscriptionLevels
        .iter()
        .map(|level| level.id)
        .collect();
    let referenced: Vec<i64> = parsed.subscriptions.iter().map(|row| row.levelId).collect();
    assert!(
        parsed
            .subscriptionLevels
            .iter()
            .any(|level| !referenced.contains(&level.id))
    );
    assert!(
        parsed
            .subscriptions
            .iter()
            .any(|row| !level_ids.contains(&row.levelId))
    );
    let again = serde_json::to_value(&parsed).unwrap();
    let text = again.to_string();
    assert!(!text.contains("ignoredField"), "{text}");
    assert!(!text.contains("fixture-extra-sub-9q"), "{text}");
    assert!(!text.contains("fixture-extra-level-9q"), "{text}");
    assert!(again.get("subscriptions").is_some());
    assert!(again.get("subscriptionLevels").is_some());
    assert!(!again["subscriptions"].is_null());
    assert!(!again["subscriptionLevels"].is_null());
    assert_eq!(again["subscriptions"][0]["endTime"], 0);
    assert_eq!(again["subscriptions"][0]["nextPayTime"], 0);
    assert!(again["subscriptions"][0]["endTime"].is_number());
    assert!(!again["subscriptions"][0]["endTime"].is_null());
    assert_eq!(again["subscriptions"][0]["payments"], "ZZZ4.00");
    assert!(again["subscriptions"][0]["id"].is_i64());
}

#[test]
fn subscriptions_missing_array_or_wrong_type_is_an_error() {
    let value: serde_json::Value = serde_json::from_str(&subscriptions_fixture()).unwrap();
    for key in ["subscriptions", "subscriptionLevels"] {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(key);
        assert!(
            serde_json::from_value::<SubscriptionsResponse>(missing).is_err(),
            "{key}"
        );
        let mut null_array = value.clone();
        null_array[key] = serde_json::Value::Null;
        assert!(
            serde_json::from_value::<SubscriptionsResponse>(null_array).is_err(),
            "{key}"
        );
    }
    let mut payments_number = value.clone();
    payments_number["subscriptions"][0]["payments"] = serde_json::json!(4);
    assert!(serde_json::from_value::<SubscriptionsResponse>(payments_number).is_err());
    for field in [
        "id",
        "ownerId",
        "levelId",
        "startTime",
        "endTime",
        "nextPayTime",
        "subscriberId",
        "payments",
    ] {
        let mut broken = value.clone();
        broken["subscriptions"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            serde_json::from_value::<SubscriptionsResponse>(broken).is_err(),
            "{field}"
        );
    }
    let empty: SubscriptionsResponse =
        serde_json::from_str(r#"{"subscriptions":[],"subscriptionLevels":[]}"#).unwrap();
    assert!(empty.subscriptions.is_empty());
    assert!(empty.subscriptionLevels.is_empty());
}

#[test]
fn fixtures_do_not_copy_contract_samples() {
    let (devapi, flow) = contracts();
    let mut secrets = header_secrets(&devapi);
    secrets.extend(header_secrets(&flow));
    assert!(!secrets.is_empty());
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for fixture_name in ["subscription_levels.json", "subscriptions.json"] {
        let text = fixture(fixture_name);
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
        for secret in &secrets {
            assert!(
                !text.contains(secret),
                "contract sample leaked into {fixture_name}"
            );
        }
    }
    let mut files = Vec::new();
    collect_text_files(&root.join("src"), &mut files);
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        for secret in &secrets {
            assert!(
                !text.contains(secret),
                "contract sample leaked into {}",
                path.display()
            );
        }
    }
}

fn collect_text_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_text_files(&path, out);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subscription_levels_get_sends_one_api_key_without_query() {
    let token = Server::spawn(Reply::Close);
    let devapi = Server::spawn(Reply::Status {
        status: 200,
        body: levels_fixture(),
    });
    let http = http_for(&token, &devapi, Duration::from_secs(5));
    let parsed = crate::levels::run_subscription_levels(Some(KEY), &http)
        .await
        .unwrap();
    assert_eq!(devapi.hits().len(), 1);
    assert_api_key_get(&devapi.hits()[0], "/v1/subscription_levels");
    assert!(token.hits().is_empty());
    assert_eq!(parsed.subscriptionLevels[0].price, "AAA1.20");
    assert!(
        devapi
            .hits()
            .iter()
            .all(|hit| hit.path == "/v1/subscription_levels")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subscriptions_get_sends_one_api_key_and_does_not_call_levels() {
    let token = Server::spawn(Reply::Close);
    let devapi = Server::spawn(Reply::Status {
        status: 200,
        body: subscriptions_fixture(),
    });
    let http = http_for(&token, &devapi, Duration::from_secs(5));
    let parsed = crate::levels::run_subscriptions(Some(KEY), &http)
        .await
        .unwrap();
    assert_eq!(devapi.hits().len(), 1);
    assert_api_key_get(&devapi.hits()[0], "/v1/subscriptions");
    assert!(
        devapi
            .hits()
            .iter()
            .all(|hit| !hit.path.contains("subscription_levels"))
    );
    assert!(token.hits().is_empty());
    assert_eq!(parsed.subscriptions[0].payments, "ZZZ4.00");
}

#[test]
fn binary_sends_only_api_key_when_token_and_client_secret_are_set() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_canary_tokens(&path);
    let prior = std::fs::read(&path).unwrap();
    for command in COMMANDS {
        let token = Server::spawn(Reply::Close);
        let devapi = Server::spawn(Reply::Status {
            status: 200,
            body: command_fixture(command),
        });
        let base = devapi_base_of(&devapi);
        let token_url = token.token_url();
        let out = run_bin(
            &[command],
            &[
                ("HOME", tmp.path().to_str().unwrap()),
                ("BOOSTY_TOKEN_PATH", path.to_str().unwrap()),
                ("BOOSTY_DEVAPI_BASE", base.as_str()),
                ("BOOSTY_TOKEN_URL", token_url.as_str()),
                ("BOOSTY_API_KEY", KEY),
                ("BOOSTY_CLIENT_ID", CLIENT_ID),
                ("BOOSTY_CLIENT_SECRET", CLIENT_SECRET),
            ],
        );
        assert_eq!(out.code, 0, "{} {}", command, out.stderr);
        assert_eq!(devapi.hits().len(), 1, "{command}");
        assert_api_key_get(&devapi.hits()[0], command_path(command));
        assert!(token.hits().is_empty(), "{command}");
        assert_eq!(std::fs::read(&path).unwrap(), prior);
        assert!(!out.stdout.contains(KEY));
        assert!(!out.stderr.contains(KEY));
        assert!(!out.stdout.contains(CLIENT_SECRET));
        assert!(!out.stderr.contains(CLIENT_SECRET));
    }
}

#[test]
fn missing_or_empty_api_key_is_auth_and_offline() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_canary_tokens(&path);
    let prior = std::fs::read(&path).unwrap();
    let canaries = [KEY, ACCESS, REFRESH, CLIENT_SECRET];
    for command in COMMANDS {
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
                vec![*command],
                vec!["--json", *command],
                vec![*command, "--json"],
            ] {
                let out = run_bin(&args, &env);
                assert_eq!(out.code, 3, "{} {:?} {}", command, args, out.stderr);
                assert!(out.stdout.is_empty(), "{command}");
                assert!(server.hits().is_empty(), "{command} {:?}", args);
                assert_eq!(std::fs::read(&path).unwrap(), prior);
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
}

#[test]
fn key_present_ignores_missing_home_and_token_path() {
    for command in COMMANDS {
        let server = Server::spawn(Reply::Status {
            status: 200,
            body: command_fixture(command),
        });
        let base = devapi_base_of(&server);
        let token_url = server.token_url();
        let out = run_bin(
            &[command],
            &[
                ("BOOSTY_API_KEY", KEY),
                ("BOOSTY_DEVAPI_BASE", base.as_str()),
                ("BOOSTY_TOKEN_URL", token_url.as_str()),
            ],
        );
        assert_eq!(out.code, 0, "{} {}", command, out.stderr);
        assert_eq!(server.hits().len(), 1, "{command}");
        assert_eq!(server.hits()[0].path, command_path(command));
    }
    let tmp = Tmp::new();
    let missing = tmp.child("absent-tokens.json");
    assert!(!missing.exists());
    for command in COMMANDS {
        let server = Server::spawn(Reply::Status {
            status: 200,
            body: command_fixture(command),
        });
        let base = devapi_base_of(&server);
        let out = run_bin(
            &[command],
            &[
                ("BOOSTY_API_KEY", KEY),
                ("BOOSTY_DEVAPI_BASE", base.as_str()),
                ("BOOSTY_TOKEN_URL", &server.token_url()),
                ("BOOSTY_TOKEN_PATH", missing.to_str().unwrap()),
            ],
        );
        assert_eq!(out.code, 0, "{} {}", command, out.stderr);
        assert!(!missing.exists(), "{command}");
    }
}

#[test]
fn json_stdout_preserves_price_and_zero() {
    for command in COMMANDS {
        let server = Server::spawn(Reply::Status {
            status: 200,
            body: command_fixture(command),
        });
        let base = devapi_base_of(&server);
        let token_url = server.token_url();
        let env = [
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", token_url.as_str()),
        ];
        for args in [vec!["--json", *command], vec![*command, "--json"]] {
            let out = run_bin(&args, &env);
            assert_eq!(out.code, 0, "{} {}", command, out.stderr);
            assert!(out.stderr.is_empty(), "{}", out.stderr);
            assert!(out.stdout.ends_with('\n'));
            assert!(!out.stdout[..out.stdout.len() - 1].contains('\n'));
            let value: serde_json::Value = serde_json::from_str(out.stdout.trim()).unwrap();
            assert!(value.is_object());
            assert!(value.get("kind").is_none());
            assert!(!out.stdout.contains("ignoredField"));
            assert!(!out.stdout.contains("fixture-extra"));
            assert!(!out.stdout.contains("null"));
            assert!(!out.stdout.contains("1970"));
            assert!(!out.stdout.contains(KEY));
            assert!(!out.stderr.contains(KEY));
            if *command == "subscription-levels" {
                let expected: SubscriptionLevelsResponse =
                    serde_json::from_str(&levels_fixture()).unwrap();
                assert_eq!(value, serde_json::to_value(&expected).unwrap());
                assert_eq!(value["subscriptionLevels"][0]["price"], "AAA1.20");
                assert_eq!(value["subscriptionLevels"][0]["id"], 4_000_000_001_i64);
                assert!(value["subscriptionLevels"][0]["id"].is_i64());
                assert!(value["subscriptionLevels"][0]["id"].as_i64().unwrap() > i32::MAX as i64);
                assert_eq!(value["subscriptionLevels"][1]["createTime"], 0);
                assert!(value["subscriptionLevels"][1]["createTime"].is_number());
                assert!(!value["subscriptionLevels"][1]["createTime"].is_string());
                assert_eq!(value["subscriptionLevels"][1]["isDeleted"], true);
                assert!(value["subscriptionLevels"][1]["isDeleted"].is_boolean());
            } else {
                let expected: SubscriptionsResponse =
                    serde_json::from_str(&subscriptions_fixture()).unwrap();
                assert_eq!(value, serde_json::to_value(&expected).unwrap());
                assert_eq!(value["subscriptions"][0]["payments"], "ZZZ4.00");
                assert_eq!(value["subscriptions"][0]["endTime"], 0);
                assert_eq!(value["subscriptions"][0]["nextPayTime"], 0);
                assert!(value["subscriptions"][0]["endTime"].is_number());
                assert!(value["subscriptions"][0]["id"].as_i64().unwrap() > i32::MAX as i64);
                assert_eq!(value["subscriptionLevels"][1]["isDeleted"], true);
                assert!(value["subscriptionLevels"][1]["isDeleted"].is_boolean());
            }
        }
    }
}

#[test]
fn human_stdout_groups_level_and_formats_price_and_time() {
    for (tz, stamp, other) in [
        ("UTC", UTC_STAMP, MOSCOW_STAMP),
        ("Europe/Moscow", MOSCOW_STAMP, UTC_STAMP),
    ] {
        let levels = run_human("subscription-levels", &levels_fixture(), tz);
        assert_eq!(levels.code, 0, "{}", levels.stderr);
        assert!(levels.stderr.is_empty(), "{}", levels.stderr);
        assert!(serde_json::from_str::<serde_json::Value>(levels.stdout.trim()).is_err());
        assert!(levels.stdout.contains("Уровни"));
        assert!(levels.stdout.contains(stamp), "{tz} {}", levels.stdout);
        assert!(!levels.stdout.contains(other), "{tz} {}", levels.stdout);
        assert!(!levels.stdout.contains("1970-01-01"), "{}", levels.stdout);
        assert!(!levels.stdout.contains("следующее смещение"));
        assert!(!levels.stdout.contains(KEY));
        assert!(!levels.stderr.contains(KEY));
        for key in ["isDeleted", "createTime", "endTime", "nextPayTime"] {
            assert!(!levels.stdout.contains(key), "{key} {}", levels.stdout);
        }
        let polka = named_block(&levels.stdout, "Polka");
        let smena = named_block(&levels.stdout, "Smena");
        let lenta = named_block(&levels.stdout, "Lenta");
        assert!(polka.find("Polka").unwrap() < polka.find("4000000001").unwrap());
        assert!(smena.contains("удалён"), "{smena}");
        assert!(!polka.contains("удалён"), "{polka}");
        assert!(!polka.contains("false"), "{polka}");
        assert!(smena.contains("нет"), "{smena}");
        assert!(polka.contains("1.20 AAA"), "{polka}");
        assert!(!polka.contains("1.2 AAA"), "{polka}");
        assert!(!polka.contains("AAA1.20"), "{polka}");
        assert!(smena.contains("price-open"), "{smena}");
        assert!(!smena.contains("-open price"), "{smena}");
        assert!(lenta.contains("AAA 1.20"), "{lenta}");
        assert!(!lenta.contains("1.20 AAA"), "{lenta}");

        let subs = run_human("subscriptions", &subscriptions_fixture(), tz);
        assert_eq!(subs.code, 0, "{}", subs.stderr);
        assert!(subs.stderr.is_empty(), "{}", subs.stderr);
        assert!(serde_json::from_str::<serde_json::Value>(subs.stdout.trim()).is_err());
        assert!(subs.stdout.contains(stamp), "{tz} {}", subs.stdout);
        assert!(!subs.stdout.contains(other), "{tz} {}", subs.stdout);
        assert!(!subs.stdout.contains("1970-01-01"));
        assert!(subs.stdout.contains("платежи: ZZZ4.00"), "{}", subs.stdout);
        assert!(!subs.stdout.contains("4.00 ZZZ"), "{}", subs.stdout);
        assert!(!subs.stdout.contains("следующее смещение"));
        assert!(!subs.stdout.contains(KEY));
        assert!(subs.stdout.contains("Orbita"));
        assert!(subs.stdout.contains("9900000099"));
        assert!(named_block(&subs.stdout, "Smena").contains("удалён"));
        assert!(!named_block(&subs.stdout, "Polka").contains("удалён"));
        assert!(subs.stdout.contains("нет"));
        for key in ["isDeleted", "createTime", "endTime", "nextPayTime"] {
            assert!(!subs.stdout.contains(key), "{key}");
        }
    }
}

fn run_human(command: &str, body: &str, tz: &str) -> Proc {
    let server = Server::spawn(Reply::Status {
        status: 200,
        body: body.to_string(),
    });
    let base = devapi_base_of(&server);
    let token_url = server.token_url();
    run_bin(
        &[command],
        &[
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", token_url.as_str()),
            ("TZ", tz),
        ],
    )
}

#[test]
fn same_level_block_in_both_commands() {
    let body = serde_json::json!({
        "subscriptionLevels": [{
            "id": 4000000099_i64,
            "price": "QQQ9.50",
            "authorId": 8100000099_i64,
            "name": "Obshchiy",
            "createTime": 0,
            "isDeleted": true
        }],
        "subscriptions": []
    })
    .to_string();
    let levels = run_human("subscription-levels", &body, "UTC");
    let subs = run_human("subscriptions", &body, "UTC");
    assert_eq!(levels.code, 0, "{}", levels.stderr);
    assert_eq!(subs.code, 0, "{}", subs.stderr);
    let block = levels
        .stdout
        .strip_prefix("Уровни\n")
        .expect("levels header");
    assert!(!block.is_empty());
    assert!(subs.stdout.contains(block), "{block}\n---\n{}", subs.stdout);
    assert_eq!(subs.stdout.matches(block).count(), 1);
    assert_eq!(subs.stdout.matches("Obshchiy").count(), 1);
    assert_eq!(subs.stdout.matches("9.50 QQQ").count(), 1);
    assert!(!levels.stdout.contains("QQQ9.50"));
    assert!(!subs.stdout.contains("QQQ9.50"));
    assert!(!levels.stdout.contains("isDeleted"));
    assert!(!subs.stdout.contains("isDeleted"));
    assert!(block.contains("удалён"));
    assert!(subs.stdout.contains("Подписки"));
    assert!(!levels.stdout.contains("Подписки"));
}

#[test]
fn empty_lists_are_success_with_nonempty_stdout() {
    let bodies = [
        (
            "subscription-levels",
            r#"{"subscriptionLevels":[]}"#,
            vec!["Уровни"],
        ),
        (
            "subscriptions",
            r#"{"subscriptions":[],"subscriptionLevels":[]}"#,
            vec!["Уровни", "Подписки"],
        ),
    ];
    for (command, body, headers) in bodies {
        let server = Server::spawn(Reply::Status {
            status: 200,
            body: body.to_string(),
        });
        let base = devapi_base_of(&server);
        let token_url = server.token_url();
        let env = [
            ("BOOSTY_API_KEY", KEY),
            ("BOOSTY_DEVAPI_BASE", base.as_str()),
            ("BOOSTY_TOKEN_URL", token_url.as_str()),
        ];
        let human = run_bin(&[command], &env);
        assert_eq!(human.code, 0, "{}", human.stderr);
        assert!(human.stderr.is_empty(), "{}", human.stderr);
        assert!(!human.stdout.is_empty());
        assert!(serde_json::from_str::<serde_json::Value>(human.stdout.trim()).is_err());
        assert!(!human.stdout.contains("нет"));
        for header in &headers {
            assert!(human.stdout.contains(header), "{header} {}", human.stdout);
        }
        for args in [vec!["--json", command], vec![command, "--json"]] {
            let out = run_bin(&args, &env);
            assert_eq!(out.code, 0, "{}", out.stderr);
            assert!(out.stderr.is_empty(), "{}", out.stderr);
            let value: serde_json::Value = serde_json::from_str(out.stdout.trim()).unwrap();
            assert!(value.get("error").is_none());
            assert!(value.get("kind").is_none());
            if command == "subscription-levels" {
                assert_eq!(value["subscriptionLevels"].as_array().unwrap().len(), 0);
            } else {
                assert_eq!(value["subscriptions"].as_array().unwrap().len(), 0);
                assert_eq!(value["subscriptionLevels"].as_array().unwrap().len(), 0);
            }
        }
    }
}

#[test]
fn devapi_401_and_403_are_api_without_bearer_fallback() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_canary_tokens(&path);
    let prior = std::fs::read(&path).unwrap();
    let canaries = [KEY, BODY_CANARY, ACCESS, REFRESH, CLIENT_SECRET];
    for command in COMMANDS {
        for status in [401_u16, 403] {
            let token = Server::spawn(Reply::Close);
            let devapi = Server::spawn(Reply::Status {
                status,
                body: format!(r#"{{"note":"{BODY_CANARY}"}}"#),
            });
            let base = devapi_base_of(&devapi);
            let token_url = token.token_url();
            let env = [
                ("HOME", tmp.path().to_str().unwrap()),
                ("BOOSTY_TOKEN_PATH", path.to_str().unwrap()),
                ("BOOSTY_DEVAPI_BASE", base.as_str()),
                ("BOOSTY_TOKEN_URL", token_url.as_str()),
                ("BOOSTY_API_KEY", KEY),
                ("BOOSTY_CLIENT_ID", CLIENT_ID),
                ("BOOSTY_CLIENT_SECRET", CLIENT_SECRET),
            ];
            for args in [
                vec!["--json", *command],
                vec![*command, "--json"],
                vec![*command],
            ] {
                let before = devapi.hits().len();
                let out = run_bin(&args, &env);
                assert_eq!(out.code, 4, "{} {} {}", command, status, out.stderr);
                assert!(out.stdout.is_empty());
                assert_eq!(devapi.hits().len(), before + 1, "{command} {status}");
                let hits = devapi.hits();
                let hit = hits.last().unwrap();
                assert!(header(hit, "authorization").is_empty(), "{:?}", hit.headers);
                assert_eq!(header(hit, "x-api-key"), vec![KEY]);
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
            }
        }
    }
}

#[test]
fn devapi_bad_body_is_api_without_second_request() {
    let cases = [
        ("subscription-levels", "not-json", "not-json"),
        ("subscription-levels", "missing", r#"{"extra":1}"#),
        (
            "subscription-levels",
            "null-array",
            r#"{"subscriptionLevels":null}"#,
        ),
        ("subscriptions", "not-json", "not-json"),
        ("subscriptions", "missing", r#"{"extra":1}"#),
        (
            "subscriptions",
            "null-subs",
            r#"{"subscriptions":null,"subscriptionLevels":[]}"#,
        ),
        (
            "subscriptions",
            "null-levels",
            r#"{"subscriptions":[],"subscriptionLevels":null}"#,
        ),
    ];
    for (command, name, body) in cases {
        let server = Server::spawn(Reply::Status {
            status: 200,
            body: body.to_string(),
        });
        let base = devapi_base_of(&server);
        let out = run_bin(
            &["--json", command],
            &[
                ("BOOSTY_API_KEY", KEY),
                ("BOOSTY_DEVAPI_BASE", base.as_str()),
                ("BOOSTY_TOKEN_URL", &server.token_url()),
            ],
        );
        assert_eq!(out.code, 4, "{command} {name} {}", out.stderr);
        assert!(out.stdout.is_empty());
        assert!(server.hits().len() <= 1, "{command} {name}");
        assert_json_fail(&out.stderr, "api", &[KEY, BODY_CANARY]);
    }
    for command in COMMANDS {
        let server = Server::spawn(Reply::Close);
        let base = devapi_base_of(&server);
        let out = run_bin(
            &["--json", command],
            &[
                ("BOOSTY_API_KEY", KEY),
                ("BOOSTY_DEVAPI_BASE", base.as_str()),
                ("BOOSTY_TOKEN_URL", &server.token_url()),
            ],
        );
        assert_eq!(out.code, 4, "{command} {}", out.stderr);
        assert!(out.stdout.is_empty());
        assert!(server.hits().len() <= 1, "{command}");
        assert_json_fail(&out.stderr, "api", &[KEY]);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn devapi_timeout_is_api_without_retry() {
    let client_src =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/client.rs"))
            .unwrap();
    let main_src =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs")).unwrap();
    assert_eq!(client_src.matches(".timeout(").count(), 1);
    assert!(main_src.contains("HTTP_TIMEOUT"));
    assert_eq!(client::HTTP_TIMEOUT, Duration::from_secs(30));
    for command in COMMANDS {
        let token = Server::spawn(Reply::Close);
        let devapi = Server::spawn(Reply::Silent);
        let http = http_for(&token, &devapi, Duration::from_millis(200));
        let started = Instant::now();
        let err = match *command {
            "subscription-levels" => crate::levels::run_subscription_levels(Some(KEY), &http)
                .await
                .unwrap_err(),
            "subscriptions" => crate::levels::run_subscriptions(Some(KEY), &http)
                .await
                .unwrap_err(),
            _ => unreachable!(),
        };
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_kind(&err, Kind::Api);
        assert!(devapi.hits().len() <= 1, "{command}");
        assert!(token.hits().is_empty(), "{command}");
        assert!(!err.english().contains(KEY));
        assert!(!format!("{err:?}").contains(KEY));
    }
}

#[test]
fn parses_json_without_content_type() {
    for command in COMMANDS {
        let server = Server::spawn(Reply::NoContentType {
            status: 200,
            body: command_fixture(command),
        });
        let base = devapi_base_of(&server);
        let out = run_bin(
            &["--json", command],
            &[
                ("BOOSTY_API_KEY", KEY),
                ("BOOSTY_DEVAPI_BASE", base.as_str()),
                ("BOOSTY_TOKEN_URL", &server.token_url()),
            ],
        );
        assert_eq!(out.code, 0, "{} {}", command, out.stderr);
        let value: serde_json::Value = serde_json::from_str(out.stdout.trim()).unwrap();
        if *command == "subscription-levels" {
            let expected: SubscriptionLevelsResponse =
                serde_json::from_str(&levels_fixture()).unwrap();
            assert_eq!(value, serde_json::to_value(&expected).unwrap());
        } else {
            let expected: SubscriptionsResponse =
                serde_json::from_str(&subscriptions_fixture()).unwrap();
            assert_eq!(value, serde_json::to_value(&expected).unwrap());
        }
    }
}

#[test]
fn bad_base_url_is_io_without_request() {
    for command in COMMANDS {
        let server = Server::spawn(Reply::Close);
        let token_url = server.token_url();
        let out = run_bin(
            &["--json", command],
            &[
                ("BOOSTY_API_KEY", KEY),
                ("BOOSTY_DEVAPI_BASE", "not a url"),
                ("BOOSTY_TOKEN_URL", token_url.as_str()),
                ("BOOSTY_AUTHORIZE_URL", token_url.as_str()),
            ],
        );
        assert_eq!(out.code, 5, "{} {}", command, out.stderr);
        assert!(out.stdout.is_empty());
        assert!(server.hits().is_empty(), "{command}");
        assert_json_fail(&out.stderr, "io", &[KEY]);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn debug_and_stderr_omit_key_and_body() {
    let token = Server::spawn(Reply::Close);
    let devapi = Server::spawn(Reply::Status {
        status: 401,
        body: format!(r#"{{"note":"{BODY_CANARY}","key":"{KEY_CANARY}"}}"#),
    });
    let http = http_for(&token, &devapi, Duration::from_secs(5));
    let err = crate::levels::run_subscription_levels(Some(KEY_CANARY), &http)
        .await
        .unwrap_err();
    let debug = format!("{err:?}");
    for canary in [KEY_CANARY, BODY_CANARY] {
        assert!(!err.english().contains(canary), "{canary}");
        assert!(!err.russian().contains(canary), "{canary}");
        assert!(!debug.contains(canary), "{canary} {debug}");
    }
    let levels_src =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/api/levels.rs"))
            .unwrap();
    let run_src =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/levels.rs"))
            .unwrap();
    assert!(!levels_src.contains("Display"));
    assert!(!run_src.contains("Display"));

    let server = Server::spawn(Reply::Status {
        status: 500,
        body: format!(r#"{{"note":"{BODY_CANARY}"}}"#),
    });
    let base = devapi_base_of(&server);
    let token_url = server.token_url();
    for command in COMMANDS {
        let out = run_bin(
            &[command],
            &[
                ("BOOSTY_API_KEY", KEY_CANARY),
                ("BOOSTY_DEVAPI_BASE", base.as_str()),
                ("BOOSTY_TOKEN_URL", token_url.as_str()),
            ],
        );
        assert_eq!(out.code, 4, "{}", out.stderr);
        assert!(out.stdout.is_empty());
        assert!(!out.stdout.contains(KEY_CANARY));
        assert!(!out.stderr.contains(KEY_CANARY));
        assert!(!out.stdout.contains(BODY_CANARY));
        assert!(!out.stderr.contains(BODY_CANARY));
        let json = run_bin(
            &["--json", command],
            &[
                ("BOOSTY_API_KEY", KEY_CANARY),
                ("BOOSTY_DEVAPI_BASE", base.as_str()),
                ("BOOSTY_TOKEN_URL", token_url.as_str()),
            ],
        );
        assert!(!json.stdout.contains(KEY_CANARY));
        assert!(!json.stderr.contains(KEY_CANARY));
        assert!(!json.stderr.contains(BODY_CANARY));
    }
}

#[test]
fn extra_positional_or_blog_or_is_free_is_usage_and_offline() {
    let server = Server::spawn(Reply::Close);
    let base = devapi_base_of(&server);
    let token_url = server.token_url();
    let tails: &[&[&str]] = &[
        &["extra"],
        &["--blog", FLAG_SECRET],
        &["--blog-id", FLAG_SECRET],
        &["--name", FLAG_SECRET],
        &["--is-free"],
        &["--is-free", "true"],
        &["--api-key", FLAG_SECRET],
    ];
    let env = [
        ("BOOSTY_API_KEY", KEY),
        ("BOOSTY_DEVAPI_BASE", base.as_str()),
        ("BOOSTY_TOKEN_URL", token_url.as_str()),
    ];
    for command in COMMANDS {
        for tail in tails {
            let mut plain = vec![*command];
            plain.extend_from_slice(tail);
            let human = run_bin(&plain, &env);
            assert_eq!(human.code, 2, "{} {}", plain.join(" "), human.stderr);
            assert!(human.stdout.is_empty());
            assert_russian_fail(&human.stderr, &[FLAG_SECRET, KEY]);
            assert!(!human.stderr.contains("Print help"));
            assert!(server.hits().is_empty(), "{}", plain.join(" "));

            let mut before = vec!["--json", *command];
            before.extend_from_slice(tail);
            let mut after = vec![*command];
            after.extend_from_slice(tail);
            after.push("--json");
            for args in [before, after] {
                let out = run_bin(&args, &env);
                assert_eq!(out.code, 2, "{} {}", args.join(" "), out.stderr);
                assert!(out.stdout.is_empty());
                assert_json_fail(&out.stderr, "usage", &[FLAG_SECRET, KEY]);
                assert!(!out.stderr.contains("Print help"));
                assert!(!out.stderr.contains("error:"));
                assert!(server.hits().is_empty(), "{}", args.join(" "));
            }
        }
    }
}

#[test]
fn auth_zero_expiry_is_not_rewritten_to_net() {
    let tmp = Tmp::new();
    let path = tmp.child("tokens.json");
    write_stored(
        &path,
        &StoredTokens {
            access_token: ACCESS.to_string(),
            refresh_token: REFRESH.to_string(),
            token_type: "Bearer".to_string(),
            expires_at: 0,
        },
    );
    let server = Server::spawn(Reply::Close);
    let out = run_bin(
        &["auth", "status"],
        &[
            ("HOME", tmp.path().to_str().unwrap()),
            ("BOOSTY_TOKEN_PATH", path.to_str().unwrap()),
            ("BOOSTY_DEVAPI_BASE", &devapi_base_of(&server)),
            ("BOOSTY_TOKEN_URL", &server.token_url()),
            ("TZ", "UTC"),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("1970-01-01 00:00:00"), "{}", out.stdout);
    assert!(!out.stdout.contains("нет"), "{}", out.stdout);
    assert!(!out.stdout.contains(ACCESS));
    assert!(!out.stdout.contains(REFRESH));
    assert!(server.hits().is_empty());
}
