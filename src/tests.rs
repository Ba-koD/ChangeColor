use std::collections::BTreeSet;

use poise::serenity_prelude as serenity;

use crate::roles::{role_position_updates, used_color_roles};
use crate::storage::{GuildConfig, LossPolicy, Store, UserColorState};
use crate::util::{
    color_role_name, is_color_role_name, is_eligible, legacy_color_role_name, normalize_hex,
};

#[test]
fn normalizes_six_digit_hex() {
    let (hex, red, green, blue) = normalize_hex("#Ff66Aa").unwrap();
    assert_eq!(hex, "#ff66aa");
    assert_eq!((red, green, blue), (255, 102, 170));
}

#[test]
fn expands_three_digit_hex() {
    let (hex, red, green, blue) = normalize_hex("0fa").unwrap();
    assert_eq!(hex, "#00ffaa");
    assert_eq!((red, green, blue), (0, 255, 170));
}

#[test]
fn rejects_invalid_hex() {
    assert!(normalize_hex("#zzzzzz").is_err());
    assert!(normalize_hex("#ffff").is_err());
}

#[test]
fn color_role_names_are_hex_only() {
    assert_eq!(color_role_name("#ff00ff"), "#ff00ff");
    assert_eq!(legacy_color_role_name("#ff00ff"), "color #ff00ff");
}

#[test]
fn recognizes_current_and_legacy_color_role_names() {
    assert!(is_color_role_name("#ff00ff"));
    assert!(is_color_role_name("color #ff00ff"));
    assert!(!is_color_role_name("관리자"));
}

#[test]
fn used_color_roles_comes_from_stored_user_state() {
    let mut config = GuildConfig::default();
    config.users.insert(
        1,
        UserColorState {
            current_role_id: Some(10),
            ..Default::default()
        },
    );
    config.users.insert(
        2,
        UserColorState {
            current_role_id: Some(20),
            ..Default::default()
        },
    );

    let role_ids = [serenity::RoleId::new(10), serenity::RoleId::new(30)]
        .into_iter()
        .collect::<BTreeSet<_>>();
    let used = used_color_roles(&config, &role_ids);

    assert!(used.contains(&serenity::RoleId::new(10)));
    assert!(!used.contains(&serenity::RoleId::new(30)));
}

#[test]
fn role_position_updates_keep_colors_between_markers() {
    let end = serenity::RoleId::new(1);
    let color = serenity::RoleId::new(2);
    let start = serenity::RoleId::new(3);

    let updates = role_position_updates(&[end, color, start], 10);

    assert_eq!(updates, vec![(end, 10), (color, 11), (start, 12)]);
}

#[test]
fn eligibility_requires_configured_role() {
    let mut config = GuildConfig::default();
    config.allowed_role_ids.insert(10);

    assert!(is_eligible(&config, &[serenity::RoleId::new(10)]));
    assert!(!is_eligible(&config, &[serenity::RoleId::new(11)]));
}

#[test]
fn empty_allowed_roles_are_not_eligible() {
    let config = GuildConfig::default();
    assert!(!is_eligible(&config, &[serenity::RoleId::new(10)]));
}

#[test]
fn store_round_trip_keeps_policy() {
    let mut store = Store::default();
    let guild = store.guild_mut(serenity::GuildId::new(1));
    guild.loss_policy = LossPolicy::RemoveAfter { grace_days: 7 };
    guild.allowed_role_ids.insert(2);

    let json = serde_json::to_string(&store).unwrap();
    let decoded: Store = serde_json::from_str(&json).unwrap();
    let decoded_guild = decoded.guild_config(serenity::GuildId::new(1));

    assert_eq!(
        decoded_guild.loss_policy,
        LossPolicy::RemoveAfter { grace_days: 7 }
    );
    assert!(decoded_guild.allowed_role_ids.contains(&2));
}
