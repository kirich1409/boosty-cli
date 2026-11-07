use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const CARGO_TOML: &str = r#"[package]
name = "boosty-cli"
version = "0.1.0"
edition = "2024"
publish = false
description = "Local CLI for the Boosty DevAPI"
repository = "https://github.com/kirich1409/boosty-cli"
readme = "README.md"
license = "MIT OR Apache-2.0"

[[bin]]
name = "boosty"
path = "src/main.rs"

[dependencies]
base64 = "0.23"
clap = { version = "4.6", features = ["derive"] }
chrono = { version = "0.4", default-features = false, features = ["clock"] }
getrandom = "0.4"
open = "5.4"
reqwest = { version = "0.13", default-features = false, features = ["form", "rustls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["io-util", "macros", "net", "rt-multi-thread", "sync", "time"] }
url = "2"
"#;

const LICENSE_MIT: &str = r#"MIT License

Copyright (c) 2026 kirich1409

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"#;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read_text(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("{}: read: {err}", path.display()))
}

fn assert_has(path: &Path, check: &str, text: &str, needle: &str) {
    assert!(
        text.contains(needle),
        "{}: {check}: missing required text",
        path.display()
    );
}

fn assert_lacks(path: &Path, check: &str, text: &str, needle: &str) {
    assert!(
        !text.contains(needle),
        "{}: {check}: forbidden text present",
        path.display()
    );
}

#[test]
fn readme_describes_existing_surface() {
    let path = root().join("README.md");
    let text = read_text(&path);
    let check = "readme_describes_existing_surface";
    for needle in [
        "Локальный CLI для Boosty DevAPI",
        "ключом приложения",
        "authorization-code flow",
        "Это не веб-API и не обещание постов, комментариев или cookie-сессии",
        "boosty auth login",
        "boosty auth refresh",
        "boosty auth status",
        "boosty user",
        "boosty subscription-levels",
        "boosty subscriptions",
        "boosty subscribers [--is-free true|false] [--limit N] [--offset N]",
        "Открыть браузер и сохранить токен",
        "Обновить токен",
        "Показать срок токена и путь к файлу",
        "Показать текущего пользователя",
        "Показать уровни подписки",
        "Показать подписки",
        "Показать подписчиков",
        "Только бесплатные при true, только платные при false",
        "Лимит записей",
        "Смещение следующей страницы",
        "Единственная интерактивная команда",
        "Агент забирает данные ключом или уже существующим файлом токенов и логин для этого не запускает",
        "не принимаются как флаг или позиционный аргумент",
        "--json` — глобальный флаг",
        "и до, и после подкоманды",
        "агент и скрипт разбирают stdout",
        "человеческий stdout — русский текст, не дамп JSON и не колонка сырых имён полей",
        "не переименовываются",
        "Порядок ключей не обещается",
        "cargo run -- --json user",
        "cargo run -- user --json",
        "BOOSTY_API_KEY",
        "BOOSTY_CLIENT_ID",
        "BOOSTY_CLIENT_SECRET",
        "BOOSTY_REDIRECT_URL",
        "BOOSTY_TOKEN_PATH",
        "XDG_CONFIG_HOME",
        "~/.config/boosty-cli/tokens.json",
        "BOOSTY_AUTHORIZE_URL",
        "BOOSTY_TOKEN_URL",
        "BOOSTY_DEVAPI_BASE",
        "X-Api-Key",
        "subscription-levels",
        "subscriptions",
        "subscribers",
        "Аргумента блога нет",
        "Authorization: Basic",
        "client_id",
        "redirect_url",
        "redirect_uri",
        "Имена параметров не сливаются",
        "Application Support",
        "не обязательная настройка обычного запуска",
        "отправляют только `X-Api-Key`",
        "отправляет только `Authorization: Bearer`",
        "принимает только Basic",
        "Один запуск делает один запрос и одну страницу",
        "Страницы внутри команды не обходятся",
        "собирает бинарник `boosty`",
        "двойной лицензии `MIT OR Apache-2.0`",
        "`LICENSE-MIT`",
        "`LICENSE-APACHE`",
    ] {
        assert_has(&path, check, &text, needle);
    }
    assert!(
        text.lines().any(|line| line == "cargo build"),
        "{}: {check}: missing line cargo build",
        path.display()
    );
    assert!(
        text.lines().any(|line| line == "cargo run --"),
        "{}: {check}: missing line cargo run --",
        path.display()
    );
    for needle in [
        "BOOSTY_API_KEY=",
        "BOOSTY_CLIENT_ID=",
        "BOOSTY_CLIENT_SECRET=",
        "BOOSTY_REDIRECT_URL=",
        "BOOSTY_TOKEN_PATH=",
        "BOOSTY_AUTHORIZE_URL=",
        "BOOSTY_TOKEN_URL=",
        "BOOSTY_DEVAPI_BASE=",
        "export BOOSTY_",
    ] {
        assert_lacks(&path, check, &text, needle);
    }
}

#[test]
fn readme_has_no_install_channel_or_ci_badge() {
    let path = root().join("README.md");
    let text = read_text(&path);
    let check = "readme_has_no_install_channel_or_ci_badge";
    // Install commands and the release URL template live in the install
    // section. crates.io and cargo binstall are named there only to refuse
    // them; release_layout rejects those as channels.
    for needle in [
        "shields.io",
        "![",
        "скач",
        "Permission is hereby granted",
        "END OF TERMS AND CONDITIONS",
        "homebrew-",
        "winget",
        "bucket add",
        "https://crates.io",
    ] {
        assert_lacks(&path, check, &text, needle);
    }
}

#[test]
fn cargo_toml_adds_only_metadata() {
    let path = root().join("Cargo.toml");
    let text = read_text(&path);
    let check = "cargo_toml_adds_only_metadata";
    assert_eq!(
        text,
        CARGO_TOML,
        "{}: {check}: bytes differ",
        path.display()
    );
    for field in [
        "description = \"Local CLI for the Boosty DevAPI\"",
        "repository = \"https://github.com/kirich1409/boosty-cli\"",
        "readme = \"README.md\"",
        "license = \"MIT OR Apache-2.0\"",
        "publish = false",
        "name = \"boosty-cli\"",
        "version = \"0.1.0\"",
        "edition = \"2024\"",
        "name = \"boosty\"",
    ] {
        assert_eq!(
            text.matches(field).count(),
            1,
            "{}: {check}: field count",
            path.display()
        );
    }
    for needle in ["license-file", "authors", "publish = true"] {
        assert_lacks(&path, check, &text, needle);
    }
}

#[test]
fn license_mit_is_standard_text() {
    let path = root().join("LICENSE-MIT");
    let text = read_text(&path);
    let check = "license_mit_is_standard_text";
    assert_eq!(
        text,
        LICENSE_MIT,
        "{}: {check}: bytes differ",
        path.display()
    );
    assert!(
        text.contains("Copyright (c) 2026 kirich1409"),
        "{}: {check}: copyright",
        path.display()
    );
    for needle in ["[year]", "[fullname]", "<YEAR>"] {
        assert_lacks(&path, check, &text, needle);
    }
}

#[test]
fn license_apache_is_standard_text() {
    let path = root().join("LICENSE-APACHE");
    let bytes = fs::read(&path).unwrap_or_else(|err| panic!("{}: read: {err}", path.display()));
    let text = read_text(&path);
    let check = "license_apache_is_standard_text";
    let copyrights = text
        .lines()
        .filter(|line| line.trim_start_matches(' ') == "Copyright 2026 kirich1409")
        .count();
    assert_eq!(
        copyrights,
        1,
        "{}: {check}: copyright line count {copyrights}",
        path.display()
    );
    for needle in [
        "Copyright [yyyy]",
        "[yyyy]",
        "name of copyright owner",
        "(c) 2026",
    ] {
        assert_lacks(&path, check, &text, needle);
    }
    for needle in [
        "Apache License",
        "Version 2.0, January 2004",
        "1. Definitions.",
        "2. Grant of Copyright License.",
        "3. Grant of Patent License.",
        "4. Redistribution.",
        "5. Submission of Contributions.",
        "6. Trademarks.",
        "7. Disclaimer of Warranty.",
        "8. Limitation of Liability.",
        "9. Accepting Warranty or Additional Liability.",
        "END OF TERMS AND CONDITIONS",
        "APPENDIX: How to apply the Apache License to your work.",
        "(Don't include",
        "the brackets!)",
    ] {
        assert_has(&path, check, &text, needle);
    }
    assert!(
        bytes.len() >= 9000,
        "{}: {check}: length {}",
        path.display(),
        bytes.len()
    );
    let notice = root().join("NOTICE");
    assert!(
        !notice.exists(),
        "{}: {check}: NOTICE exists",
        notice.display()
    );
}

#[test]
fn contract_samples_absent() {
    let root = root();
    let contracts = [
        root.join("devapi.md"),
        root.join("authorization_code_flow.md"),
    ];
    let contract = contracts
        .iter()
        .map(|path| read_text(path))
        .collect::<Vec<_>>()
        .join("\n");
    let samples = collect_samples(&contract);
    for path in walk_files(root) {
        if contracts.iter().any(|contract_path| contract_path == &path) {
            continue;
        }
        let bytes = fs::read(&path).unwrap_or_else(|err| panic!("{}: read: {err}", path.display()));
        for sample in &samples {
            if contains_bytes(&bytes, sample.value.as_bytes()) {
                panic!(
                    "{}: contract_samples_absent: kind {} length {}",
                    path.display(),
                    sample.kinds,
                    sample.value.len()
                );
            }
        }
    }
}

struct Sample {
    value: String,
    kinds: String,
}

fn collect_samples(text: &str) -> Vec<Sample> {
    let mut by_value: BTreeMap<String, BTreeSet<&'static str>> = BTreeMap::new();
    let mut add = |kind: &'static str, values: BTreeSet<String>| {
        for value in values {
            by_value.entry(value).or_default().insert(kind);
        }
    };

    let code = accepted(marker_values(text, "code=", false));
    let api_key = accepted(marker_values(text, "X-Api-Key:", true));
    let basic = accepted(marker_values(text, "Basic ", false));
    let bearer = accepted(marker_values(text, "Bearer ", false));
    let access = accepted(
        json_quoted_values(text, "access_token")
            .into_iter()
            .filter(|value| value != "string"),
    );
    let refresh = accepted(
        json_quoted_values(text, "refresh_token")
            .into_iter()
            .filter(|value| value != "string"),
    );
    let secret = accepted(quoted_after_word(text, "secret"));
    let client_eq = accepted(marker_values(text, "client_id=", false));
    let client_quoted = accepted(quoted_after_word(text, "client\\_id"));
    require_kind("code", &code, 1);
    require_kind("api_key", &api_key, 1);
    require_kind("basic", &basic, 2);
    require_kind("bearer", &bearer, 1);
    require_kind("access", &access, 1);
    require_kind("refresh", &refresh, 1);
    require_kind("secret", &secret, 1);
    require_kind("client_id=", &client_eq, 1);
    require_kind("client_id quotes", &client_quoted, 1);

    add("code", code);
    add("api_key", api_key);
    add("basic", basic);
    add("bearer", bearer);
    add("access", access);
    add("refresh", refresh);
    add("secret", secret);
    add("client_id", client_eq);
    let mut client_search = client_quoted;
    // Replace `\_` only inside a value that was actually extracted. Do not
    // rewrite the contract text before the other extractors run.
    for value in client_search.clone() {
        if value.contains("\\_") {
            let normalized = value.replace("\\_", "_");
            if accepted(std::iter::once(normalized.clone()))
                .into_iter()
                .next()
                .is_some()
            {
                client_search.insert(normalized);
            }
        }
    }
    add("client_id", client_search);

    by_value
        .into_iter()
        .map(|(value, kinds)| Sample {
            kinds: kinds.into_iter().collect::<Vec<_>>().join(","),
            value,
        })
        .collect()
}

fn require_kind(kind: &str, values: &BTreeSet<String>, min: usize) {
    let lengths = values.iter().map(String::len).collect::<Vec<_>>();
    assert!(
        values.len() >= min,
        "contract_samples_absent: kind {kind} count {} lengths {lengths:?}",
        values.len()
    );
}

fn accepted(values: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    values
        .into_iter()
        .filter(|value| is_sample(value))
        .collect()
}

fn is_sample(value: &str) -> bool {
    value.len() >= 16
        && !value
            .chars()
            .any(|ch| matches!(ch, '"' | '\'' | '&') || ch.is_whitespace())
}

fn marker_values(text: &str, marker: &str, skip_one_space: bool) -> Vec<String> {
    let mut values = Vec::new();
    let mut start = 0;
    while let Some(rel) = text[start..].find(marker) {
        let at = start + rel;
        let mut value_at = at + marker.len();
        if skip_one_space && text.as_bytes().get(value_at) == Some(&b' ') {
            value_at += 1;
        }
        values.push(take_value(text, value_at).to_string());
        start = at + marker.len();
    }
    values
}

fn take_value(text: &str, start: usize) -> &str {
    let Some(rest) = text.get(start..) else {
        return "";
    };
    let end = rest
        .char_indices()
        .find(|(_, ch)| is_terminator(*ch))
        .map_or(rest.len(), |(index, _)| index);
    &rest[..end]
}

fn is_terminator(ch: char) -> bool {
    matches!(
        ch,
        ' ' | '\t' | '\n' | '"' | '\'' | '`' | '&' | '#' | '\\' | '<' | '>' | '(' | ')' | ','
    )
}

fn quoted_after_word(text: &str, word: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut start = 0;
    while let Some(rel) = text[start..].find(word) {
        let at = start + rel;
        let mut cursor = at + word.len();
        while text
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
        {
            cursor += 1;
        }
        if text.as_bytes().get(cursor) == Some(&b'"') {
            let open = cursor + 1;
            if let Some(rel_end) = text[open..].find('"') {
                values.push(text[open..open + rel_end].to_string());
            }
        }
        start = at + word.len();
    }
    values
}

fn json_quoted_values(text: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\"");
    let mut values = Vec::new();
    let mut start = 0;
    while let Some(rel) = text[start..].find(&needle) {
        let at = start + rel;
        let mut cursor = at + needle.len();
        cursor = skip_json_space(text, cursor);
        if text.as_bytes().get(cursor) == Some(&b':') {
            cursor = skip_json_space(text, cursor + 1);
            if text.as_bytes().get(cursor) == Some(&b'"') {
                let open = cursor + 1;
                if let Some(rel_end) = text[open..].find('"') {
                    values.push(text[open..open + rel_end].to_string());
                }
            }
        }
        start = at + needle.len();
    }
    values
}

fn skip_json_space(text: &str, mut index: usize) -> usize {
    while text
        .as_bytes()
        .get(index)
        .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
    {
        index += 1;
    }
    index
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let entries =
            fs::read_dir(&dir).unwrap_or_else(|err| panic!("{}: read_dir: {err}", dir.display()));
        for entry in entries {
            let entry = entry.unwrap_or_else(|err| panic!("{}: entry: {err}", dir.display()));
            let path = entry.path();
            let name = entry.file_name();
            if path.is_dir() {
                if name == ".git" || name == "target" {
                    continue;
                }
                dirs.push(path);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
    files
}
