use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const TEMPLATE: &str =
    "https://github.com/kirich1409/boosty-cli/releases/download/v{version}/boosty-{target}.{ext}";

const CI_YML: &str = r#"name: ci

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

permissions:
  contents: read

jobs:
  ci:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          persist-credentials: false
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: stable
          components: rustfmt, clippy
      - run: cargo fmt --check
      - run: cargo clippy --all-targets --all-features -- -D warnings
      - run: cargo test
"#;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read_text(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("{}: read: {err}", path.display()))
}

fn package_version(text: &str) -> String {
    let mut found = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("version = \"") {
            let Some(version) = rest.strip_suffix('"') else {
                panic!("unterminated version line");
            };
            found.push(version.to_string());
        }
    }
    assert_eq!(found.len(), 1, "expected one package version line");
    found.remove(0)
}

fn archive_url(version: &str, target: &str, ext: &str) -> String {
    format!(
        "https://github.com/kirich1409/boosty-cli/releases/download/v{version}/boosty-{target}.{ext}"
    )
}

fn has_hex64(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes
        .windows(64)
        .any(|window| window.iter().all(|byte| byte.is_ascii_hexdigit()))
}

fn assert_has(path: &Path, text: &str, needle: &str) {
    assert!(
        text.contains(needle),
        "{}: missing required text {needle}",
        path.display()
    );
}

fn assert_lacks(path: &Path, text: &str, needle: &str) {
    assert!(
        !text.contains(needle),
        "{}: forbidden text present {needle}",
        path.display()
    );
}

#[test]
fn workflows_keep_ci_and_add_release() {
    let dir = root().join(".github/workflows");
    let mut names = fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("{}: {err}", dir.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|err| panic!("{}: entry: {err}", dir.display()))
                .file_name()
                .into_string()
                .unwrap_or_else(|name| panic!("non-utf8 workflow name {name:?}"))
        })
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, vec!["ci.yml".to_string(), "release.yml".to_string()]);

    let ci_path = dir.join("ci.yml");
    let ci = read_text(&ci_path);
    assert_eq!(ci, CI_YML, "{}: bytes changed", ci_path.display());
    assert_has(&ci_path, &ci, "actions/checkout@v7");
    assert_has(&ci_path, &ci, "persist-credentials: false");
    assert_has(&ci_path, &ci, "contents: read");
    assert_has(&ci_path, &ci, "cargo fmt --check");
    assert_has(
        &ci_path,
        &ci,
        "cargo clippy --all-targets --all-features -- -D warnings",
    );
    assert_has(&ci_path, &ci, "cargo test");
    assert_lacks(&ci_path, &ci, "gh release");
    assert_lacks(&ci_path, &ci, "secrets");
}

#[test]
fn release_workflow_is_a_tag_job_not_ci() {
    let path = root().join(".github/workflows/release.yml");
    let text = read_text(&path);
    assert_has(&path, &text, "v[0-9]+.[0-9]+.[0-9]+");
    assert_has(&path, &text, "workflow_dispatch:");
    assert_lacks(&path, &text, "inputs:");
    assert_lacks(&path, &text, "branches:");
    assert_lacks(&path, &text, "pull_request:");
    assert_lacks(&path, &text, "schedule:");
    assert_eq!(text.matches("runs-on:").count(), 1);
    assert_has(&path, &text, "runs-on: macos-latest");
    assert_lacks(&path, &text, "matrix:");
    assert_has(&path, &text, "contents: write");
    assert_has(&path, &text, "actions/checkout@v7");
    assert_has(&path, &text, "fetch-depth: 0");
    assert_has(&path, &text, "persist-credentials: true");
    assert_has(&path, &text, "dtolnay/rust-toolchain@stable");
    assert_has(&path, &text, "toolchain: stable");
    assert_lacks(&path, &text, "rust-toolchain@1");
    assert_lacks(&path, &text, "toolchain: \"1");
    assert_lacks(&path, &text, "toolchain: 1");

    for needle in [
        "cargo fmt",
        "cargo clippy",
        "cargo test",
        "cargo publish",
        "secrets",
        "winget",
        "boosty.to",
        "homebrew-",
        "CARGO_PKG_VERSION",
        "--force",
        "amend",
        "gh pr",
        "--clobber",
        "cargo binstall",
        "crates.io",
        "publish = true",
    ] {
        assert_lacks(&path, &text, needle);
    }
}

#[test]
fn release_workflow_refuses_a_mismatched_tag_before_build() {
    let path = root().join(".github/workflows/release.yml");
    let text = read_text(&path);
    let create_at = text
        .find("gh release create")
        .unwrap_or_else(|| panic!("{}: missing gh release create", path.display()));
    let before = &text[..create_at];
    assert!(
        before.contains("github.ref_type"),
        "tag comparison is not before gh release create"
    );
    assert!(before.contains("!= \"tag\""));
    assert!(before.contains("branch or a ref without a tag"));
    assert!(before.contains("^v[0-9]+\\.[0-9]+\\.[0-9]+$"));
    assert!(before.contains("version = \""));
    assert!(before.contains("Cargo.toml"));
    assert!(before.contains("v${version}"));
    assert!(before.contains("exit 1"));
    let match_at = before.find("github.ref_type").expect("ref type check");
    let view_at = before
        .find("gh release view")
        .expect("existing release check");
    let build_at = before.find("cargo build").expect("build step");
    assert!(match_at < view_at && view_at < build_at);
}

#[test]
fn release_workflow_packs_five_distinct_archives() {
    let path = root().join(".github/workflows/release.yml");
    let text = read_text(&path);
    let pairs = [
        ("x86_64-unknown-linux-gnu", "boosty-linux-x86_64.tar.gz"),
        ("aarch64-unknown-linux-gnu", "boosty-linux-aarch64.tar.gz"),
        ("aarch64-apple-darwin", "boosty-macos-aarch64.tar.gz"),
        ("x86_64-apple-darwin", "boosty-macos-x86_64.tar.gz"),
        ("x86_64-pc-windows-gnu", "boosty-windows-x86_64.zip"),
    ];
    for (triple, archive) in pairs {
        assert_has(&path, &text, triple);
        assert_has(&path, &text, archive);
        assert!(
            text.contains(&format!("--target {triple}")),
            "{}: missing a distinct --target {triple}",
            path.display()
        );
    }
    assert_has(&path, &text, "SHA256SUMS");
    assert_has(&path, &text, "shasum -a 256");
    assert_has(&path, &text, "chmod +x");
    assert_has(&path, &text, "tar -C");
    assert_has(&path, &text, "boosty.exe");
    assert_has(&path, &text, "zip -j");
    assert_eq!(text.matches("release-notes.sh").count(), 1);
    assert_eq!(text.matches("${RUNNER_TEMP}/release-notes.md").count(), 3);
    assert_has(&path, &text, "gh release create");
    assert_has(&path, &text, "--notes-file");
    assert_has(&path, &text, ">> CHANGELOG.md");
    assert_has(&path, &text, "## ${RELEASE_VERSION}");
    assert_has(
        &path,
        &text,
        "git add CHANGELOG.md Formula/boosty.rb bucket/boosty.json",
    );
    assert_eq!(text.matches("git add").count(), 1);
    assert_has(&path, &text, "git push origin main");
    assert_has(&path, &text, "github-actions[bot]");
    let create_at = text.find("gh release create").expect("create");
    let append_at = text.find(">> CHANGELOG.md").expect("append");
    let section_guard = text.find("## ${RELEASE_VERSION}").expect("section guard");
    assert!(create_at < section_guard && section_guard < append_at);
}

#[test]
fn release_notes_script_reads_local_git_only() {
    let path = root().join("scripts/release-notes.sh");
    let text = read_text(&path);
    for needle in [
        "curl",
        "wget",
        "git fetch",
        "git push",
        "http://",
        "https://",
    ] {
        assert_lacks(&path, &text, needle);
    }
    assert_has(&path, &text, "git tag --merged HEAD");
    assert_has(&path, &text, "git rev-list --count");
    assert_has(&path, &text, "--reverse --format=%s");
}

#[test]
fn release_notes_include_history_when_no_previous_tag() {
    let repo = TempRepo::new("no-previous");
    repo.commit("oldest subject");
    repo.commit("newer subject");
    repo.tag("v0.1");
    repo.tag("not-a-release");
    repo.tag("v0.1.0");
    let text = repo.notes("0.1.0");
    assert_eq!(text, "## 0.1.0\noldest subject\nnewer subject\n");
}

#[test]
fn release_notes_exclude_previous_tag_subject() {
    let repo = TempRepo::new("previous");
    repo.commit("root subject");
    repo.tag("v0.1.0");
    repo.commit("after previous");
    repo.commit("head subject");
    repo.tag("v0.2.0");
    let text = repo.notes("0.2.0");
    assert_eq!(text, "## 0.2.0\nafter previous\nhead subject\n");
    assert!(!text.contains("root subject"));
}

#[test]
fn release_notes_skip_non_release_tags_and_keep_subject() {
    let repo = TempRepo::new("nearest");
    repo.commit("older release");
    repo.tag("v1.0.0");
    repo.commit("nearer release");
    repo.tag("v1.1.0");
    repo.commit("beta commit");
    repo.tag("v0.1.0-beta");
    repo.commit("short tag commit");
    repo.tag("v0.1");
    repo.commit("arbitrary tag commit");
    repo.tag("arbitrary-tag");
    repo.commit_with_body("feat: keep me", "BODY_NOT_IN_NOTES");
    repo.tag("v1.2.0");
    let hash = repo.git_stdout(&["rev-parse", "HEAD"]);
    let text = repo.notes("1.2.0");
    assert_eq!(
        text,
        "## 1.2.0\nbeta commit\nshort tag commit\narbitrary tag commit\nfeat: keep me\n"
    );
    assert!(!text.contains("older release"));
    assert!(!text.contains("nearer release"));
    assert!(!text.contains("BODY_NOT_IN_NOTES"));
    assert!(!text.contains(hash.trim()));
    assert!(!text.contains("нет изменений"));
    assert!(!text.contains("no changes"));
}

#[test]
fn release_notes_empty_range_is_header_only() {
    let repo = TempRepo::new("empty");
    repo.commit("only subject");
    repo.tag("v0.1.0");
    let text = repo.notes("0.2.0");
    assert_eq!(text, "## 0.2.0\n");
    assert!(!text.contains("only subject"));
    assert!(!text.contains("нет изменений"));
    assert!(!text.contains("no changes"));
}

#[test]
fn install_channels_share_one_release_template() {
    let version = package_version(&read_text(&root().join("Cargo.toml")));
    assert_eq!(version, "0.1.0");
    let readme_path = root().join("README.md");
    let formula_path = root().join("Formula/boosty.rb");
    let manifest_path = root().join("bucket/boosty.json");
    let readme = read_text(&readme_path);
    let formula = read_text(&formula_path);
    let manifest = read_text(&manifest_path);
    let workflow = read_text(&root().join(".github/workflows/release.yml"));

    for (path, text) in [
        (&readme_path, &readme),
        (&formula_path, &formula),
        (&manifest_path, &manifest),
    ] {
        assert_has(path, text, TEMPLATE);
        assert_lacks(path, text, "homebrew-");
        assert_lacks(path, text, "winget");
        assert_lacks(path, text, "publish = true");
        assert!(!has_hex64(text), "{}: invented checksum", path.display());
    }
    assert_lacks(&formula_path, &formula, "cargo binstall");
    assert_lacks(&formula_path, &formula, "crates.io");
    assert_lacks(&manifest_path, &manifest, "cargo binstall");
    assert_lacks(&manifest_path, &manifest, "crates.io");
    assert_lacks(
        &root().join(".github/workflows/release.yml"),
        &workflow,
        "cargo binstall",
    );
    assert_lacks(
        &root().join(".github/workflows/release.yml"),
        &workflow,
        "crates.io",
    );

    let unix = [
        ("macos-aarch64", "tar.gz"),
        ("macos-x86_64", "tar.gz"),
        ("linux-aarch64", "tar.gz"),
        ("linux-x86_64", "tar.gz"),
    ];
    for (target, ext) in unix {
        let url = archive_url(&version, target, ext);
        assert_has(&formula_path, &formula, &url);
        assert_lacks(&manifest_path, &manifest, &url);
    }
    let windows = archive_url(&version, "windows-x86_64", "zip");
    assert_has(&manifest_path, &manifest, &windows);
    assert_lacks(&formula_path, &formula, &windows);
    assert!(formula.contains(&format!("version \"{version}\"")));
    assert!(manifest.contains(&format!("\"version\": \"{version}\"")));
    assert_has(&formula_path, &formula, "license \"MIT\"");
    assert_has(&manifest_path, &manifest, "\"license\": \"MIT\"");
    assert_has(&manifest_path, &manifest, "\"bin\": \"boosty.exe\"");
    assert!(manifest.contains(&format!("\"##\": \"{TEMPLATE}\"")));
    assert_eq!(manifest.matches("\"url\"").count(), 1);
    assert_eq!(formula.matches("url \"").count(), 4);
    assert_lacks(&manifest_path, &manifest, "\"hash\"");
    assert_lacks(&manifest_path, &manifest, "sha1:");
    assert_lacks(&manifest_path, &manifest, "md5:");
    assert_lacks(&manifest_path, &manifest, "32bit");
    assert_lacks(&manifest_path, &manifest, "arm64");
    assert_lacks(&formula_path, &formula, "sha256");
    assert_lacks(&formula_path, &formula, "bottle");
    assert_lacks(&formula_path, &formula, "windows");
    assert_lacks(&formula_path, &formula, ".exe");
    assert!(formula.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with('#') && trimmed.contains(TEMPLATE)
    }));
    assert_eq!(formula.matches("on_macos").count(), 1);
    assert_eq!(formula.matches("on_linux").count(), 1);
    assert_eq!(formula.matches("on_arm").count(), 2);
    assert_eq!(formula.matches("on_intel").count(), 2);
    let install_at = formula.find("def install").expect("install");
    let install_end = formula[install_at..]
        .find("\nend")
        .map(|end| install_at + end)
        .expect("install end");
    let install = &formula[install_at..install_end];
    assert!(install.contains("bin.install \"boosty\""));
    assert!(!install.contains("on_"));
    assert!(!root().join("HomebrewFormula").exists());
    assert!(!root().join("boosty.rb").exists());
    assert!(formula_path.is_file());
}

#[test]
fn readme_install_section_names_one_version() {
    let version = package_version(&read_text(&root().join("Cargo.toml")));
    let path = root().join("README.md");
    let text = read_text(&path);
    let build = text.find("## Сборка из исходников").expect("build");
    let install = text.find("## Установка").expect("install");
    let license = text.find("## Лицензия").expect("license");
    assert!(build < install && install < license);
    let section = &text[install..license];
    assert!(section.contains(&version));
    assert!(section.contains(&format!("v{version}")));
    assert!(section.contains(TEMPLATE));
    for line in [
        "brew tap kirich1409/boosty-cli https://github.com/kirich1409/boosty-cli",
        "brew install kirich1409/boosty-cli/boosty",
        "scoop install https://raw.githubusercontent.com/kirich1409/boosty-cli/main/bucket/boosty.json",
        "cargo install --git https://github.com/kirich1409/boosty-cli",
    ] {
        assert!(
            text.lines().any(|candidate| candidate == line),
            "{}: missing install line {line}",
            path.display()
        );
    }
    assert!(
        !text
            .lines()
            .any(|line| line == "brew tap kirich1409/boosty-cli"),
        "one-argument tap invents a homebrew tap"
    );
    assert_eq!(text.matches("brew ").count(), 2);
    assert!(!text.contains("--tag"));
    assert!(
        text.contains("`cargo binstall` и crates.io не обещаны, потому что `publish = false`.")
    );
    assert!(!text.lines().any(|line| line.starts_with("cargo binstall")));
    assert_lacks(&path, &text, "https://crates.io");
    assert_lacks(&path, &text, "publish = true");
    assert_lacks(&path, &text, "homebrew-");
    assert_lacks(&path, &text, "scoop bucket add");
    assert_lacks(&path, &text, "winget");
    assert_lacks(&path, &text, "GitHub Releases");
    assert_lacks(&path, &text, "shields.io");
    assert_lacks(&path, &text, "![");
    assert_lacks(&path, &text, "скач");
}

#[test]
fn changelog_has_no_unreleased_section() {
    let path = root().join("CHANGELOG.md");
    let text = read_text(&path);
    assert!(!text.lines().any(|line| line == "## 0.1.0"));
    assert!(text.contains("Секции дописывает релиз"));
    assert!(text.contains("Старые секции не переписываются"));
}

#[test]
fn agents_release_rule_keeps_command_contract() {
    let path = root().join("AGENTS.md");
    let text = read_text(&path);
    assert_has(
        &path,
        &text,
        "A release tag is `v` plus `version` from Cargo.toml, and the two sides match character for character.",
    );
    assert_has(&path, &text, "publish = false");
    assert_has(
        &path,
        &text,
        "Install from this repository, not from a separate tap.",
    );
    for needle in [
        "boosty auth login",
        "auth refresh",
        "auth status",
        "user",
        "subscription-levels",
        "subscriptions",
        "subscribers",
        "--json",
    ] {
        assert_has(&path, &text, needle);
    }
}

struct TempRepo {
    path: PathBuf,
}

static NEXT_REPO: AtomicU64 = AtomicU64::new(0);

impl TempRepo {
    fn new(label: &str) -> Self {
        let n = NEXT_REPO.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "boosty-release-notes-{}-{label}-{n}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
        let repo = Self { path };
        repo.git(&["init", "-b", "main"]);
        repo
    }

    fn git(&self, args: &[&str]) {
        let output = self.command(args);
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_stdout(&self, args: &[&str]) -> String {
        let output = self.command(args);
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("git stdout")
    }

    fn command(&self, args: &[&str]) -> std::process::Output {
        Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Release Test")
            .env("GIT_AUTHOR_EMAIL", "release-test@example.com")
            .env("GIT_COMMITTER_NAME", "Release Test")
            .env("GIT_COMMITTER_EMAIL", "release-test@example.com")
            .output()
            .unwrap_or_else(|err| panic!("git: {err}"))
    }

    fn commit(&self, subject: &str) {
        self.git(&["commit", "--allow-empty", "-m", subject]);
    }

    fn commit_with_body(&self, subject: &str, body: &str) {
        self.git(&["commit", "--allow-empty", "-m", subject, "-m", body]);
    }

    fn tag(&self, name: &str) {
        self.git(&["tag", name]);
    }

    fn notes(&self, version: &str) -> String {
        let script = root().join("scripts/release-notes.sh");
        let output = Command::new("sh")
            .arg(&script)
            .arg(version)
            .current_dir(&self.path)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap_or_else(|err| panic!("release-notes.sh: {err}"));
        assert!(
            output.status.success(),
            "release-notes.sh failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("notes stdout")
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
