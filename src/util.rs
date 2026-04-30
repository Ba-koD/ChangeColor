use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use poise::serenity_prelude as serenity;

use crate::storage::{GuildConfig, LossPolicy};
use crate::{Error, user_error};

pub(crate) fn highest_role<'a>(
    roles: &'a HashMap<serenity::RoleId, serenity::Role>,
    member_roles: &[serenity::RoleId],
) -> Option<&'a serenity::Role> {
    member_roles
        .iter()
        .filter_map(|role_id| roles.get(role_id))
        .max_by_key(|role| (role.position, std::cmp::Reverse(role.id.get())))
}

pub(crate) fn is_eligible(config: &GuildConfig, roles: &[serenity::RoleId]) -> bool {
    !config.allowed_role_ids.is_empty()
        && roles
            .iter()
            .any(|role_id| config.allowed_role_ids.contains(&role_id.get()))
}

pub(crate) fn is_managed_color_role(config: &GuildConfig, role_id: serenity::RoleId) -> bool {
    config.marker_start_role_id == Some(role_id.get())
        || config.marker_end_role_id == Some(role_id.get())
        || config
            .color_roles
            .values()
            .any(|configured_id| *configured_id == role_id.get())
}

pub(crate) fn color_role_name(hex: &str) -> String {
    hex.to_string()
}

pub(crate) fn legacy_color_role_name(hex: &str) -> String {
    format!("color {hex}")
}

pub(crate) fn is_color_role_name(name: &str) -> bool {
    normalize_hex(name).is_ok()
        || name
            .strip_prefix("color ")
            .is_some_and(|hex| normalize_hex(hex).is_ok())
}

pub(crate) fn normalize_hex(input: &str) -> Result<(String, u8, u8, u8), Error> {
    let value = input.trim().strip_prefix('#').unwrap_or(input.trim());
    let expanded = match value.len() {
        3 => value.chars().flat_map(|ch| [ch, ch]).collect::<String>(),
        6 => value.to_string(),
        _ => {
            return Err(user_error(
                "HEX 색상은 `#rrggbb` 또는 `#rgb` 형식이어야 합니다.",
            ));
        }
    };

    if !expanded.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(user_error(
            "HEX 색상에는 0-9, a-f 문자만 사용할 수 있습니다.",
        ));
    }

    let red = u8::from_str_radix(&expanded[0..2], 16)?;
    let green = u8::from_str_radix(&expanded[2..4], 16)?;
    let blue = u8::from_str_radix(&expanded[4..6], 16)?;
    Ok((
        format!("#{expanded}", expanded = expanded.to_lowercase()),
        red,
        green,
        blue,
    ))
}

pub(crate) fn mention_role(role_id: serenity::RoleId) -> String {
    format!("<@&{}>", role_id.get())
}

pub(crate) fn format_policy(policy: &LossPolicy) -> String {
    match policy {
        LossPolicy::Keep => "유지".to_string(),
        LossPolicy::RemoveImmediate => "즉시제거".to_string(),
        LossPolicy::RemoveAfter { grace_days } => format!("유예제거({grace_days}일)"),
    }
}

pub(crate) fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
