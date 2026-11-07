use std::path::Path;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_boosty")
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(args: &[&str], env: &[(&str, &str)]) -> Out {
    let mut cmd = Command::new(bin());
    cmd.args(args).env_clear();
    for (key, value) in env {
        cmd.env(key, value);
    }
    let out = cmd.output().expect("run boosty");
    Out {
        code: out.status.code().unwrap(),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    }
}

fn temp_home() -> tempfile_dir::Tmp {
    tempfile_dir::Tmp::new()
}

mod tempfile_dir {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    pub struct Tmp(PathBuf);

    impl Tmp {
        pub fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "boosty-cli-proc-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn write_tokens(path: &Path, expires_at: i64) {
    std::fs::write(
        path,
        format!(
            r#"{{"access_token":"access-test-1","refresh_token":"refresh-test-1","token_type":"TestType","expires_at":{expires_at}}}"#
        ),
    )
    .unwrap();
}

#[test]
fn json_before_and_after_subcommand() {
    let home = temp_home();
    let path = home.path().join("tokens.json");
    write_tokens(&path, 1_700_000_000);
    let env = [
        ("HOME", home.path().to_str().unwrap()),
        ("BOOSTY_TOKEN_PATH", path.to_str().unwrap()),
        ("TZ", "UTC"),
    ];
    for args in [
        vec!["--json", "auth", "status"],
        vec!["auth", "status", "--json"],
    ] {
        let out = run(&args, &env);
        assert_eq!(out.code, 0, "{} {}", args.join(" "), out.stderr);
        assert!(out.stderr.is_empty(), "{}", out.stderr);
        let value: serde_json::Value = serde_json::from_str(out.stdout.trim()).unwrap();
        assert_eq!(value["expires_at"], 1_700_000_000);
        assert!(out.stdout.contains(path.to_str().unwrap()));
    }
    let missing = home.path().join("missing.json");
    let env = [
        ("HOME", home.path().to_str().unwrap()),
        ("BOOSTY_TOKEN_PATH", missing.to_str().unwrap()),
    ];
    for args in [
        vec!["--json", "auth", "status"],
        vec!["auth", "status", "--json"],
    ] {
        let out = run(&args, &env);
        assert_eq!(out.code, 3, "{}", out.stderr);
        assert!(out.stdout.is_empty());
        let value: serde_json::Value = serde_json::from_str(out.stderr.trim()).unwrap();
        assert_eq!(value["kind"], "auth");
        assert!(value["error"].as_str().unwrap().is_ascii());
        assert_eq!(out.stderr.trim().lines().count(), 1);
    }
}

#[test]
fn human_success_is_russian_not_json() {
    let home = temp_home();
    let path = home.path().join("tokens.json");
    write_tokens(&path, 1_700_000_000);
    let out = run(
        &["auth", "status"],
        &[
            ("HOME", home.path().to_str().unwrap()),
            ("BOOSTY_TOKEN_PATH", path.to_str().unwrap()),
            ("TZ", "UTC"),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(serde_json::from_str::<serde_json::Value>(out.stdout.trim()).is_err());
    assert!(
        out.stdout.contains("Срок: 2023-11-14 22:13:20"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("Файл:"));
    assert!(out.stdout.contains(path.to_str().unwrap()));
    assert!(!out.stdout.contains("access_token"));
    assert!(!out.stdout.contains("refresh_token"));
    assert!(!out.stdout.contains("token_type"));
    assert!(!out.stdout.contains("access-test-1"));
    assert!(!out.stdout.contains("refresh-test-1"));
}

#[test]
fn help_is_russian_not_clap_english_template() {
    for args in ["--help", "auth", "login"] {
        let _ = args;
    }
    let pages = [
        run(&["--help"], &[]),
        run(&["auth", "--help"], &[]),
        run(&["auth", "login", "--help"], &[]),
    ];
    let joined = pages
        .iter()
        .map(|page| page.stdout.clone())
        .collect::<Vec<_>>()
        .join("\n");
    for needle in [
        "Клиент Boosty DevAPI",
        "Вход и токен",
        "Открыть браузер и сохранить токен",
        "Обновить токен",
        "Показать срок токена и путь к файлу",
        "Печатать JSON на stdout",
        "Показать справку",
        "Использование",
        "Команды",
        "Параметры",
    ] {
        assert!(
            joined.contains(needle) || pages.iter().any(|page| page.stdout.contains(needle)),
            "{needle}\n{joined}"
        );
    }
    assert!(pages[0].stdout.contains("Клиент Boosty DevAPI"));
    assert!(pages[0].stdout.contains("Вход и токен"));
    assert!(
        pages[1]
            .stdout
            .contains("Открыть браузер и сохранить токен")
    );
    assert!(pages[1].stdout.contains("Обновить токен"));
    assert!(
        pages[1]
            .stdout
            .contains("Показать срок токена и путь к файлу")
    );
    assert!(
        pages[2]
            .stdout
            .contains("Открыть браузер и сохранить токен")
    );
    for page in &pages {
        assert_eq!(page.code, 0, "{}", page.stderr);
        assert!(!page.stdout.contains("Print help"));
        assert!(
            !page
                .stdout
                .contains("Print this message or the help of the given subcommand(s)")
        );
        assert!(!page.stderr.contains("Print help"));
    }
    assert!(pages[0].stdout.contains("Показать текущего пользователя"));
    let user_help = run(&["user", "--help"], &[]);
    assert_eq!(user_help.code, 0, "{}", user_help.stderr);
    assert!(user_help.stdout.contains("Показать текущего пользователя"));
    assert!(!user_help.stdout.contains("Print help"));
    assert!(
        !user_help
            .stdout
            .contains("Print this message or the help of the given subcommand(s)")
    );
    assert!(!user_help.stderr.contains("Print help"));
}

#[test]
fn extra_positional_or_secret_flag_is_usage() {
    let cases = [
        vec!["auth", "login", "extra"],
        vec!["auth", "status", "--token", "secret"],
        vec!["auth", "refresh", "--refresh-token", "x"],
        vec!["user", "extra"],
        vec!["user", "--token", "secret"],
        vec!["user", "--api-key", "secret"],
    ];
    for args in cases {
        let out = run(&args, &[]);
        assert_eq!(out.code, 2, "{} {}", args.join(" "), out.stderr);
        assert!(out.stdout.is_empty());
        assert!(out.stderr.contains('а') || out.stderr.contains('Н'));
        assert!(serde_json::from_str::<serde_json::Value>(out.stderr.trim()).is_err());
        assert!(!out.stderr.contains("secret"));
    }
    for args in [
        vec!["--json", "auth", "login", "extra"],
        vec!["auth", "status", "--token", "secret", "--json"],
        vec!["--json", "auth", "refresh", "--refresh-token", "x"],
        vec!["auth", "refresh", "--json", "--refresh-token", "x"],
        vec!["--json", "user", "extra"],
        vec!["user", "--json", "extra"],
        vec!["--json", "user", "--token", "secret"],
        vec!["user", "--token", "secret", "--json"],
        vec!["--json", "user", "--api-key", "secret"],
        vec!["user", "--api-key", "secret", "--json"],
    ] {
        let out = run(&args, &[]);
        assert_eq!(out.code, 2, "{} {}", args.join(" "), out.stderr);
        assert!(out.stdout.is_empty());
        let value: serde_json::Value = serde_json::from_str(out.stderr.trim()).unwrap();
        assert_eq!(value["kind"], "usage");
        assert!(!out.stderr.contains("Print help"));
        assert!(!out.stderr.contains("error:"));
        assert!(!out.stderr.contains("secret"));
        assert_eq!(out.stderr.trim().lines().count(), 1);
    }
}

#[test]
fn devapi_override_is_parsed_and_status_sends_no_request() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let home = temp_home();
    let path = home.path().join("tokens.json");
    write_tokens(&path, 1_700_000_000);
    let out = run(
        &["auth", "status"],
        &[
            ("HOME", home.path().to_str().unwrap()),
            ("BOOSTY_TOKEN_PATH", path.to_str().unwrap()),
            ("BOOSTY_DEVAPI_BASE", &format!("http://{addr}/v1")),
            (
                "BOOSTY_TOKEN_URL",
                &format!("http://{addr}/oauth/server/token"),
            ),
            ("TZ", "UTC"),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(listener.accept().is_err());
}

#[test]
fn process_edge_has_one_tokio_runtime_and_no_blocking_client() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut mains = 0;
    let needle_main = ["#[", "tokio::main]"].join("");
    let needle_blocking = ["reqwest::", "blocking"].join("");
    for entry in std::fs::read_dir(&src).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            for nested in std::fs::read_dir(&path).unwrap() {
                let nested = nested.unwrap().path();
                let text = std::fs::read_to_string(&nested).unwrap_or_default();
                assert!(!text.contains(&needle_blocking), "{}", nested.display());
                mains += text.matches(&needle_main).count();
            }
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains(&needle_blocking), "{}", path.display());
        if text.contains(&needle_main) {
            assert_eq!(path.file_name().unwrap(), "main.rs");
            mains += text.matches(&needle_main).count();
        }
    }
    assert_eq!(mains, 1);
}
