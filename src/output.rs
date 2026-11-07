use std::io::{self, Write};
use std::path::Path;

use chrono::{DateTime, Local};

use crate::error::Fail;

#[derive(Debug)]
pub(crate) struct SessionView {
    pub expires_at: i64,
    pub path: std::path::PathBuf,
}

pub(crate) fn render_success(json: bool, view: &SessionView) -> String {
    if json {
        render_json(view)
    } else {
        render_human(view)
    }
}

pub(crate) fn render_fail(json: bool, fail: &Fail) -> String {
    if json {
        let value = serde_json::json!({
            "error": fail.english(),
            "kind": fail.kind().as_str(),
        });
        format!("{value}\n")
    } else {
        format!("{}\n", fail.russian())
    }
}

pub(crate) fn print_success(json: bool, view: &SessionView) {
    let _ = write_stdout(&render_success(json, view));
}

pub(crate) fn print_fail(json: bool, fail: &Fail) {
    let _ = write_stderr(&render_fail(json, fail));
}

pub(crate) fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn render_human(view: &SessionView) -> String {
    format!(
        "Срок: {}\nФайл: {}\n",
        local_stamp(view.expires_at),
        path_string(&view.path)
    )
}

fn render_json(view: &SessionView) -> String {
    let value = serde_json::json!({
        "expires_at": view.expires_at,
        "path": path_string(&view.path),
    });
    format!("{value}\n")
}

fn local_stamp(expires_at: i64) -> String {
    DateTime::from_timestamp(expires_at, 0)
        .map(|utc| {
            utc.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| expires_at.to_string())
}

fn write_stdout(text: &str) -> io::Result<()> {
    let mut out = io::stdout().lock();
    out.write_all(text.as_bytes())?;
    out.flush()
}

fn write_stderr(text: &str) -> io::Result<()> {
    let mut err = io::stderr().lock();
    err.write_all(text.as_bytes())?;
    err.flush()
}
