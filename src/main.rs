mod api;
mod auth;
mod cli;
mod client;
mod config;
mod error;
mod levels;
mod output;
mod subscribers;
mod user;

#[cfg(test)]
mod tests;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use clap::error::ErrorKind;

use crate::auth::{LoginDeps, LoginRequest, SocketBinder};
use crate::cli::{AuthCommand, Command};
use crate::error::Fail;

enum Ran {
    Session(output::SessionView),
    User(api::user::UserResponse),
    SubscriptionLevels(api::levels::SubscriptionLevelsResponse),
    Subscriptions(api::levels::SubscriptionsResponse),
    Subscribers(api::subscribers::SubscribersResponse),
}

#[tokio::main]
async fn main() {
    let code = dispatch().await;
    let _ = io::stdout().flush();
    let _ = io::stderr().flush();
    std::process::exit(code);
}

async fn dispatch() -> i32 {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let json_flag = argv.iter().any(|arg| arg == "--json");
    let parsed = match cli::try_parse() {
        Ok(cli) => cli,
        Err(error) if error.kind() == ErrorKind::DisplayHelp => {
            let _ = error.print();
            return 0;
        }
        Err(_) => {
            output::print_fail(json_flag, &Fail::usage());
            return Fail::usage().code();
        }
    };
    match run(parsed.command).await {
        Ok(Ran::Session(view)) => {
            output::print_success(parsed.json, &view);
            0
        }
        Ok(Ran::User(body)) => {
            output::print_user(parsed.json, &body);
            0
        }
        Ok(Ran::SubscriptionLevels(body)) => {
            output::print_subscription_levels(parsed.json, &body);
            0
        }
        Ok(Ran::Subscriptions(body)) => {
            output::print_subscriptions(parsed.json, &body);
            0
        }
        Ok(Ran::Subscribers(body)) => {
            output::print_subscribers(parsed.json, &body);
            0
        }
        Err(fail) => {
            output::print_fail(parsed.json, &fail);
            fail.code()
        }
    }
}

async fn run(command: Command) -> Result<Ran, Fail> {
    let env = config::Env::from_process();
    let hosts = client::resolve_hosts(
        env.authorize_url.as_deref(),
        env.token_url.as_deref(),
        env.devapi_base.as_deref(),
    )?;
    let authorize = hosts.authorize.clone();
    let http = client::ApiClient::build(client::HTTP_TIMEOUT, hosts)?;
    match command {
        Command::Auth { action, .. } => {
            let path = resolve_token_path(&env)?;
            let view = match action {
                AuthCommand::Login { .. } => {
                    auth::run_login(
                        LoginRequest {
                            client_id: env.client_id,
                            client_secret: env.client_secret,
                            redirect_url: env.redirect_url.unwrap_or_default(),
                            authorize_endpoint: authorize,
                            http,
                            token_path: path,
                            timeout: auth::LOGIN_TIMEOUT,
                        },
                        LoginDeps {
                            opener: system_open,
                            binder: SocketBinder,
                            rename: rename_file,
                            clock: SystemTime::now,
                        },
                        &mut io::stderr(),
                    )
                    .await
                }
                AuthCommand::Refresh { .. } => {
                    auth::run_refresh(
                        env.client_id.as_deref(),
                        env.client_secret.as_deref(),
                        &path,
                        &http,
                        SystemTime::now,
                        rename_file,
                    )
                    .await
                }
                AuthCommand::Status { .. } => auth::run_status(&path),
            };
            view.map(Ran::Session)
        }
        Command::User { .. } => {
            let path = resolve_token_path(&env)?;
            user::run_user(
                env.client_id.as_deref(),
                env.client_secret.as_deref(),
                &path,
                &http,
                SystemTime::now,
                rename_file,
            )
            .await
            .map(Ran::User)
        }
        Command::SubscriptionLevels { .. } => {
            levels::run_subscription_levels(env.api_key.as_deref(), &http)
                .await
                .map(Ran::SubscriptionLevels)
        }
        Command::Subscriptions { .. } => levels::run_subscriptions(env.api_key.as_deref(), &http)
            .await
            .map(Ran::Subscriptions),
        Command::Subscribers {
            is_free,
            limit,
            offset,
            ..
        } => subscribers::run_subscribers(env.api_key.as_deref(), &http, is_free, limit, offset)
            .await
            .map(Ran::Subscribers),
    }
}

fn resolve_token_path(env: &config::Env) -> Result<PathBuf, Fail> {
    config::token_path(
        env.token_path.as_deref(),
        env.xdg_config_home.as_deref(),
        env.home.as_deref(),
    )
}

fn rename_file(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::rename(from, to)
}

fn system_open(url: &str) -> io::Result<()> {
    open::that_detached(url)
}
