use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use poise::serenity_prelude as serenity;

use crate::storage::{GuildConfig, LossPolicy};
use crate::{Error, user_error};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ColorSpec {
    Solid(ColorValue),
    Gradient { start: ColorValue, end: ColorValue },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ColorValue {
    pub(crate) hex: String,
    pub(crate) red: u8,
    pub(crate) green: u8,
    pub(crate) blue: u8,
}

impl ColorSpec {
    pub(crate) fn solid(input: &str) -> Result<Self, Error> {
        Ok(Self::Solid(ColorValue::parse(input)?))
    }

    pub(crate) fn gradient(start: &str, end: &str) -> Result<Self, Error> {
        Ok(Self::Gradient {
            start: ColorValue::parse(start)?,
            end: ColorValue::parse(end)?,
        })
    }

    pub(crate) fn parse_key(input: &str) -> Result<Self, Error> {
        if let Some((start, end)) = input.split_once('-') {
            Self::gradient(start, end)
        } else {
            Self::solid(input)
        }
    }

    pub(crate) fn key(&self) -> String {
        match self {
            Self::Solid(color) => color.hex.clone(),
            Self::Gradient { start, end } => format!("{}-{}", start.hex, end.hex),
        }
    }

    pub(crate) fn role_name(&self) -> String {
        self.key()
    }

    pub(crate) fn display(&self) -> String {
        self.key()
    }

    pub(crate) fn primary(&self) -> &ColorValue {
        match self {
            Self::Solid(color) => color,
            Self::Gradient { start, .. } => start,
        }
    }

    pub(crate) fn secondary(&self) -> Option<&ColorValue> {
        match self {
            Self::Solid(_) => None,
            Self::Gradient { end, .. } => Some(end),
        }
    }

    pub(crate) fn is_gradient(&self) -> bool {
        matches!(self, Self::Gradient { .. })
    }
}

impl ColorValue {
    fn parse(input: &str) -> Result<Self, Error> {
        let (hex, red, green, blue) = normalize_hex(input)?;
        Ok(Self {
            hex,
            red,
            green,
            blue,
        })
    }

    pub(crate) fn as_role_colour(&self) -> serenity::Colour {
        serenity::Colour::from_rgb(self.red, self.green, self.blue)
    }

    pub(crate) fn as_discord_int(&self) -> u32 {
        u32::from(self.red) << 16 | u32::from(self.green) << 8 | u32::from(self.blue)
    }
}

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

pub(crate) fn legacy_color_role_name(hex: &str) -> String {
    format!("color {hex}")
}

pub(crate) fn is_color_role_name(name: &str) -> bool {
    ColorSpec::parse_key(name).is_ok()
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
