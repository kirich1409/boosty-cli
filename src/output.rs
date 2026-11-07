use std::io::{self, Write};
use std::path::Path;

use chrono::{DateTime, Local};

use crate::api::levels::{
    Subscription, SubscriptionLevel, SubscriptionLevelsResponse, SubscriptionsResponse,
};
use crate::api::subscribers::{SubscriberSubscription, SubscriberUser, SubscribersResponse};
use crate::api::user::UserResponse;
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

pub(crate) fn render_user(json: bool, body: &UserResponse) -> String {
    if json {
        let text = serde_json::to_string(body).expect("user response serializes");
        format!("{text}\n")
    } else {
        render_user_human(body)
    }
}

pub(crate) fn print_user(json: bool, body: &UserResponse) {
    let _ = write_stdout(&render_user(json, body));
}

pub(crate) fn render_subscription_levels(json: bool, body: &SubscriptionLevelsResponse) -> String {
    if json {
        let text = serde_json::to_string(body).expect("subscription levels response serializes");
        format!("{text}\n")
    } else {
        render_levels_section(&body.subscriptionLevels)
    }
}

pub(crate) fn print_subscription_levels(json: bool, body: &SubscriptionLevelsResponse) {
    let _ = write_stdout(&render_subscription_levels(json, body));
}

pub(crate) fn render_subscriptions(json: bool, body: &SubscriptionsResponse) -> String {
    if json {
        let text = serde_json::to_string(body).expect("subscriptions response serializes");
        format!("{text}\n")
    } else {
        let mut text = render_levels_section(&body.subscriptionLevels);
        text.push_str("Подписки\n");
        let blocks = join_blocks(body.subscriptions.iter().map(render_subscription));
        if !blocks.is_empty() {
            text.push_str(&blocks);
            text.push('\n');
        }
        text
    }
}

pub(crate) fn print_subscriptions(json: bool, body: &SubscriptionsResponse) {
    let _ = write_stdout(&render_subscriptions(json, body));
}

pub(crate) fn render_subscribers(json: bool, body: &SubscribersResponse) -> String {
    if json {
        let text = serde_json::to_string(body).expect("subscribers response serializes");
        format!("{text}\n")
    } else {
        render_subscribers_human(body)
    }
}

pub(crate) fn print_subscribers(json: bool, body: &SubscribersResponse) {
    let _ = write_stdout(&render_subscribers(json, body));
}

pub(crate) fn print_fail(json: bool, fail: &Fail) {
    let _ = write_stderr(&render_fail(json, fail));
}

pub(crate) fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn render_user_human(body: &UserResponse) -> String {
    let user = &body.user;
    format!(
        "Имя: {}\nНомер: {}\nПочта: {}\nАватар: {}\nАвтор: {}\nЗаблокирован: {}\n",
        user.name,
        user.id,
        user.email,
        user.avatarUrl,
        yes_no(user.flags.isAuthor),
        yes_no(user.flags.isBanned),
    )
}

fn yes_no(flag: bool) -> &'static str {
    if flag { "да" } else { "нет" }
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

fn render_levels_section(levels: &[SubscriptionLevel]) -> String {
    let mut text = String::from("Уровни\n");
    let blocks = join_blocks(levels.iter().map(render_level));
    if !blocks.is_empty() {
        text.push_str(&blocks);
        text.push('\n');
    }
    text
}

fn render_level(level: &SubscriptionLevel) -> String {
    let mut lines = vec![
        level.name.clone(),
        format!("номер: {}", level.id),
        format!("цена: {}", display_price(&level.price)),
        format!("создан: {}", display_time(level.createTime)),
    ];
    if level.isDeleted {
        lines.push("удалён".to_string());
    }
    lines.join("\n")
}

fn render_subscribers_human(body: &SubscribersResponse) -> String {
    let mut text = String::from("Подписчики\n");
    let users = join_blocks(body.users.iter().map(render_subscriber_user));
    if !users.is_empty() {
        text.push_str(&users);
        text.push('\n');
    }
    text.push_str("Подписки\n");
    let subscriptions = join_blocks(
        body.subscriptions
            .iter()
            .map(render_subscriber_subscription),
    );
    if !subscriptions.is_empty() {
        text.push_str(&subscriptions);
        text.push('\n');
    }
    if let Some(extra) = &body.extra {
        let offset = extra.nextOffset;
        text.push_str(&format!("следующее смещение: {offset}\n"));
    }
    text
}

fn render_subscriber_user(user: &SubscriberUser) -> String {
    format!(
        "Имя: {}\nНомер: {}\nПочта: {}\nАватар: {}\nАвтор: {}\nЗаблокирован: {}",
        user.name,
        user.id,
        user.email,
        user.avatarUrl,
        yes_no(user.flags.isAuthor),
        yes_no(user.flags.isBanned),
    )
}

fn render_subscriber_subscription(subscription: &SubscriberSubscription) -> String {
    [
        format!("номер: {}", subscription.id),
        format!("владелец: {}", subscription.ownerId),
        format!("уровень: {}", subscription.levelId),
        format!("подписчик: {}", subscription.subscriberId),
        format!("начало: {}", display_time(subscription.startTime)),
        format!("конец: {}", display_time(subscription.endTime)),
        format!(
            "следующая оплата: {}",
            display_time(subscription.nextPayTime)
        ),
        format!("платежи: {}", subscription.payments),
    ]
    .join("\n")
}

fn render_subscription(subscription: &Subscription) -> String {
    [
        format!("номер: {}", subscription.id),
        format!("владелец: {}", subscription.ownerId),
        format!("уровень: {}", subscription.levelId),
        format!("подписчик: {}", subscription.subscriberId),
        format!("начало: {}", display_time(subscription.startTime)),
        format!("конец: {}", display_time(subscription.endTime)),
        format!(
            "следующая оплата: {}",
            display_time(subscription.nextPayTime)
        ),
        format!("платежи: {}", subscription.payments),
    ]
    .join("\n")
}

fn join_blocks(blocks: impl Iterator<Item = String>) -> String {
    let mut text = String::new();
    for (index, block) in blocks.enumerate() {
        if index > 0 {
            text.push_str("\n\n");
        }
        text.push_str(&block);
    }
    text
}

fn display_price(price: &str) -> String {
    let prefix_len = price
        .bytes()
        .position(|byte| !byte.is_ascii_alphabetic())
        .unwrap_or(price.len());
    if prefix_len == 0 || prefix_len == price.len() {
        return price.to_string();
    }
    let amount = &price[prefix_len..];
    if is_amount(amount) {
        format!("{amount} {}", &price[..prefix_len])
    } else {
        price.to_string()
    }
}

fn is_amount(value: &str) -> bool {
    let mut split = value.split('.');
    let Some(whole) = split.next() else {
        return false;
    };
    if whole.is_empty() || !whole.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    match split.next() {
        None => true,
        Some(fraction) => {
            !fraction.is_empty()
                && fraction.bytes().all(|byte| byte.is_ascii_digit())
                && split.next().is_none()
        }
    }
}

// Zero is «нет» only here. Auth expiry still goes through local_stamp.
fn display_time(stamp: i64) -> String {
    if stamp == 0 {
        return "нет".to_string();
    }
    DateTime::from_timestamp(stamp, 0)
        .map(|utc| {
            utc.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| stamp.to_string())
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
