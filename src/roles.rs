use std::collections::{BTreeSet, HashMap};

use poise::serenity_prelude as serenity;

use crate::config::{END_MARKER_NAME, ROLE_LIMIT, START_MARKER_NAME};
use crate::discord::current_bot_member;
use crate::storage::{GuildConfig, LossPolicy, Storage};
use crate::util::{
    color_role_name, highest_role, is_color_role_name, is_eligible, legacy_color_role_name,
    mention_role, normalize_hex, now_unix,
};
use crate::{Error, user_error};

pub(crate) async fn apply_color_for_user(
    ctx: &serenity::Context,
    storage: &Storage,
    guild_id: serenity::GuildId,
    user_id: serenity::UserId,
    raw_hex: &str,
    check_eligibility: bool,
) -> Result<String, Error> {
    let (hex, red, green, blue) = normalize_hex(raw_hex)?;
    let member = guild_id.member(ctx, user_id).await?;
    let config = storage.guild_config(guild_id).await;

    if check_eligibility && !is_eligible(&config, &member.roles) {
        if config.allowed_role_ids.is_empty() {
            return Err(user_error(
                "아직 `/컬러설정 허용역할추가`로 사용 가능 역할이 설정되지 않았습니다.",
            ));
        }
        return Err(user_error("컬러 명령어를 사용할 수 있는 역할이 없습니다."));
    }

    let role_id = ensure_color_role(ctx, storage, guild_id, &hex, red, green, blue).await?;
    let removed_role_ids =
        remove_configured_color_roles(ctx, &config, &member, Some(role_id)).await?;
    if !member.roles.contains(&role_id) {
        member.add_role(&ctx.http, role_id).await?;
    }

    storage
        .update_guild(guild_id, |guild| {
            let state = guild.users.entry(user_id.get()).or_default();
            state.last_hex = Some(hex.clone());
            state.current_role_id = Some(role_id.get());
            state.lost_eligibility_at = None;
        })
        .await?;

    cleanup_unused_color_roles(ctx, storage, guild_id, removed_role_ids).await;

    Ok(hex)
}

pub(crate) async fn remove_user_color(
    ctx: &serenity::Context,
    storage: &Storage,
    guild_id: serenity::GuildId,
    user_id: serenity::UserId,
    mark_lost: bool,
) -> Result<bool, Error> {
    let member = guild_id.member(ctx, user_id).await?;
    let config = storage.guild_config(guild_id).await;
    let removed_role_ids = remove_configured_color_roles(ctx, &config, &member, None).await?;
    let removed = !removed_role_ids.is_empty();
    let now = now_unix();

    storage
        .update_guild(guild_id, |guild| {
            let state = guild.users.entry(user_id.get()).or_default();
            state.current_role_id = None;
            if mark_lost {
                state.lost_eligibility_at = Some(now);
            }
        })
        .await?;

    cleanup_unused_color_roles(ctx, storage, guild_id, removed_role_ids).await;

    Ok(removed)
}

pub(crate) async fn restore_user_color(
    ctx: &serenity::Context,
    storage: &Storage,
    guild_id: serenity::GuildId,
    user_id: serenity::UserId,
    check_eligibility: bool,
) -> Result<String, Error> {
    let config = storage.guild_config(guild_id).await;
    let Some(last_hex) = config
        .users
        .get(&user_id.get())
        .and_then(|state| state.last_hex.clone())
    else {
        return Err(user_error("저장된 마지막 컬러가 없습니다."));
    };

    apply_color_for_user(
        ctx,
        storage,
        guild_id,
        user_id,
        &last_hex,
        check_eligibility,
    )
    .await
}

async fn ensure_color_role(
    ctx: &serenity::Context,
    storage: &Storage,
    guild_id: serenity::GuildId,
    hex: &str,
    red: u8,
    green: u8,
    blue: u8,
) -> Result<serenity::RoleId, Error> {
    ensure_markers(ctx, storage, guild_id).await?;

    let roles = guild_id.roles(&ctx.http).await?;
    let config = storage.guild_config(guild_id).await;
    if let Some(role_id) = config.color_roles.get(hex).copied() {
        let role_id = serenity::RoleId::new(role_id);
        if let Some(role) = roles.get(&role_id) {
            sync_color_role(ctx, guild_id, role, hex, red, green, blue).await?;
            return Ok(role_id);
        }
    }

    let role_name = color_role_name(hex);
    if let Some(role) = roles
        .values()
        .find(|role| role.name == role_name || role.name == legacy_color_role_name(hex))
    {
        let role_id = role.id;
        sync_color_role(ctx, guild_id, role, hex, red, green, blue).await?;
        storage
            .update_guild(guild_id, |guild| {
                guild.color_roles.insert(hex.to_string(), role_id.get());
            })
            .await?;
        return Ok(role_id);
    }

    if roles.len() >= ROLE_LIMIT {
        return Err(user_error(format!(
            "서버 역할 수가 Discord 제한({ROLE_LIMIT})에 도달해 새 컬러 역할을 만들 수 없습니다."
        )));
    }

    let bot_member = current_bot_member(ctx, guild_id).await?;
    let bot_highest_position = highest_role(&roles, &bot_member.roles)
        .ok_or_else(|| user_error("봇 최고 역할을 확인할 수 없습니다."))?;

    let role = guild_id
        .create_role(
            &ctx.http,
            serenity::EditRole::new()
                .name(role_name)
                .permissions(serenity::Permissions::empty())
                .colour(serenity::Colour::from_rgb(red, green, blue))
                .hoist(false)
                .mentionable(false),
        )
        .await?;
    if role.position >= bot_highest_position.position {
        return Err(user_error(
            "새 컬러 역할이 봇 최고 역할보다 낮게 생성되지 않았습니다. 봇 역할 위치를 올려주세요.",
        ));
    }

    storage
        .update_guild(guild_id, |guild| {
            guild.color_roles.insert(hex.to_string(), role.id.get());
        })
        .await?;

    let config = storage.guild_config(guild_id).await;
    if config.anchor_role_id.is_some() {
        reposition_color_block(ctx, storage, guild_id).await?;
    }

    Ok(role.id)
}

async fn sync_color_role(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
    role: &serenity::Role,
    hex: &str,
    red: u8,
    green: u8,
    blue: u8,
) -> Result<(), Error> {
    let expected_name = color_role_name(hex);
    let expected_colour = serenity::Colour::from_rgb(red, green, blue);
    if role.name != expected_name || role.colour != expected_colour {
        guild_id
            .edit_role(
                &ctx.http,
                role.id,
                serenity::EditRole::new()
                    .name(expected_name)
                    .colour(expected_colour)
                    .permissions(serenity::Permissions::empty())
                    .hoist(false)
                    .mentionable(false),
            )
            .await?;
    }

    Ok(())
}

pub(crate) async fn ensure_markers(
    ctx: &serenity::Context,
    storage: &Storage,
    guild_id: serenity::GuildId,
) -> Result<(serenity::RoleId, serenity::RoleId), Error> {
    let config = storage.guild_config(guild_id).await;
    let roles = guild_id.roles(&ctx.http).await?;
    let start_id = ensure_marker_role(
        ctx,
        guild_id,
        &roles,
        config.marker_start_role_id,
        START_MARKER_NAME,
    )
    .await?;
    let roles = guild_id.roles(&ctx.http).await?;
    let end_id = ensure_marker_role(
        ctx,
        guild_id,
        &roles,
        config.marker_end_role_id,
        END_MARKER_NAME,
    )
    .await?;

    storage
        .update_guild(guild_id, |guild| {
            guild.marker_start_role_id = Some(start_id.get());
            guild.marker_end_role_id = Some(end_id.get());
        })
        .await?;

    Ok((start_id, end_id))
}

async fn ensure_marker_role(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
    roles: &HashMap<serenity::RoleId, serenity::Role>,
    configured_id: Option<u64>,
    name: &str,
) -> Result<serenity::RoleId, Error> {
    if let Some(role_id) = configured_id.map(serenity::RoleId::new) {
        if roles.contains_key(&role_id) {
            return Ok(role_id);
        }
    }

    if let Some(role) = roles.values().find(|role| role.name == name) {
        return Ok(role.id);
    }

    if roles.len() >= ROLE_LIMIT {
        return Err(user_error(format!(
            "서버 역할 수가 Discord 제한({ROLE_LIMIT})에 도달해 marker 역할을 만들 수 없습니다."
        )));
    }

    let role = guild_id
        .create_role(
            &ctx.http,
            serenity::EditRole::new()
                .name(name)
                .permissions(serenity::Permissions::empty())
                .colour(serenity::Colour::default())
                .hoist(false)
                .mentionable(false),
        )
        .await?;
    Ok(role.id)
}

pub(crate) async fn reposition_color_block(
    ctx: &serenity::Context,
    storage: &Storage,
    guild_id: serenity::GuildId,
) -> Result<String, Error> {
    let config = storage.guild_config(guild_id).await;
    let Some(anchor_id) = config.anchor_role_id.map(serenity::RoleId::new) else {
        return Ok("위치기준이 설정되어 있지 않아 재정렬을 건너뛰었습니다.".to_string());
    };
    let Some(start_id) = config.marker_start_role_id.map(serenity::RoleId::new) else {
        return Err(user_error("COLOR START marker가 없습니다."));
    };
    let Some(end_id) = config.marker_end_role_id.map(serenity::RoleId::new) else {
        return Err(user_error("COLOR END marker가 없습니다."));
    };

    let roles = guild_id.roles(&ctx.http).await?;
    let anchor_role = roles
        .get(&anchor_id)
        .ok_or_else(|| user_error("저장된 위치기준 역할을 찾을 수 없습니다."))?;
    let bot_member = current_bot_member(ctx, guild_id).await?;
    let bot_highest = highest_role(&roles, &bot_member.roles)
        .ok_or_else(|| user_error("봇 최고 역할을 확인할 수 없습니다."))?;

    if anchor_role.position >= bot_highest.position {
        return Err(user_error(
            "위치기준 역할이 봇 최고 역할보다 높거나 같습니다. 봇 역할을 더 위로 올려주세요.",
        ));
    }

    let color_role_ids = color_role_ids_for_block(&config, &roles, start_id, end_id);
    let color_role_id_set = color_role_ids.iter().copied().collect::<BTreeSet<_>>();
    let mut bottom_to_top = Vec::new();
    bottom_to_top.push(end_id);
    bottom_to_top.extend(color_role_ids);
    bottom_to_top.push(start_id);

    for role_id in &bottom_to_top {
        let Some(role) = roles.get(role_id) else {
            return Err(user_error(
                "컬러 역할 묶음 안의 역할이 삭제되어 재정렬할 수 없습니다.",
            ));
        };
        if role.id == anchor_id || role.position >= bot_highest.position {
            return Err(user_error(
                "컬러 역할 묶음 안에 봇이 관리할 수 없는 역할이 있습니다. 봇 역할을 더 위로 올려주세요.",
            ));
        }
    }

    let mut base_position = anchor_role.position.saturating_add(1);
    for _ in 0..roles.len().saturating_add(1) {
        validate_block_target_position(base_position, bottom_to_top.len(), bot_highest.position)?;
        let role_positions = role_position_updates(&bottom_to_top, base_position);
        edit_role_positions_bulk(guild_id, &role_positions).await?;

        let updated_roles = guild_id.roles(&ctx.http).await?;
        let intruders =
            non_color_roles_between_markers(&updated_roles, start_id, end_id, &color_role_id_set);
        if intruders.is_empty() {
            return Ok(format!(
                "컬러 역할 묶음 역할 {}개를 {} 바로 위로 이동했습니다.",
                bottom_to_top.len(),
                mention_role(anchor_id)
            ));
        }

        let Some(next_base_position) = intruders
            .iter()
            .filter_map(|role_id| updated_roles.get(role_id))
            .map(|role| role.position.saturating_add(1))
            .max()
        else {
            break;
        };
        if next_base_position <= base_position {
            break;
        }
        base_position = next_base_position;
    }

    Err(user_error(
        "COLOR START와 COLOR END 사이에서 외부 역할을 빼내지 못했습니다. 봇 역할 위치를 더 위로 올린 뒤 다시 시도하세요.",
    ))
}

async fn edit_role_positions_bulk(
    guild_id: serenity::GuildId,
    role_positions: &[(serenity::RoleId, u16)],
) -> Result<(), Error> {
    let token = std::env::var("DISCORD_TOKEN")
        .map_err(|_| user_error("DISCORD_TOKEN env var is required"))?;
    let body = role_positions
        .iter()
        .map(|(role_id, position)| {
            serde_json::json!({
                "id": role_id.get().to_string(),
                "position": position,
            })
        })
        .collect::<Vec<_>>();
    let url = format!(
        "https://discord.com/api/v10/guilds/{}/roles",
        guild_id.get()
    );
    let response = reqwest::Client::new()
        .patch(url)
        .header("Authorization", format!("Bot {token}"))
        .json(&body)
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_else(|_| "".to_string());
        return Err(user_error(format!(
            "역할 위치를 재정렬하지 못했습니다: HTTP {status} {text}"
        )));
    }

    Ok(())
}

fn color_role_ids_for_block(
    config: &GuildConfig,
    roles: &HashMap<serenity::RoleId, serenity::Role>,
    start_id: serenity::RoleId,
    end_id: serenity::RoleId,
) -> Vec<serenity::RoleId> {
    let mut candidate_ids = config
        .color_roles
        .values()
        .copied()
        .map(serenity::RoleId::new)
        .filter(|role_id| *role_id != start_id && *role_id != end_id)
        .filter(|role_id| roles.contains_key(role_id))
        .collect::<BTreeSet<_>>();

    for role in roles.values() {
        if role.id != start_id
            && role.id != end_id
            && !role.managed
            && is_color_role_name(&role.name)
        {
            candidate_ids.insert(role.id);
        }
    }

    let mut color_roles = candidate_ids
        .into_iter()
        .filter_map(|role_id| roles.get(&role_id))
        .collect::<Vec<_>>();
    color_roles.sort_by(|left, right| {
        left.position
            .cmp(&right.position)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.get().cmp(&right.id.get()))
    });

    color_roles.into_iter().map(|role| role.id).collect()
}

fn non_color_roles_between_markers(
    roles: &HashMap<serenity::RoleId, serenity::Role>,
    start_id: serenity::RoleId,
    end_id: serenity::RoleId,
    color_role_ids: &BTreeSet<serenity::RoleId>,
) -> Vec<serenity::RoleId> {
    let (Some(start_role), Some(end_role)) = (roles.get(&start_id), roles.get(&end_id)) else {
        return Vec::new();
    };
    let low_position = start_role.position.min(end_role.position);
    let high_position = start_role.position.max(end_role.position);

    let mut intruders = roles
        .values()
        .filter(|role| role.id != start_id && role.id != end_id)
        .filter(|role| role.position > low_position && role.position < high_position)
        .filter(|role| !color_role_ids.contains(&role.id))
        .map(|role| role.id)
        .collect::<Vec<_>>();
    intruders.sort_by_key(|role_id| {
        roles
            .get(role_id)
            .map(|role| (role.position, role.id.get()))
            .unwrap_or_default()
    });
    intruders
}

fn validate_block_target_position(
    base_position: u16,
    block_len: usize,
    bot_highest_position: u16,
) -> Result<(), Error> {
    let top_position = base_position as usize + block_len.saturating_sub(1);
    if top_position >= bot_highest_position as usize {
        return Err(user_error(
            "컬러 역할 묶음이 봇 최고 역할보다 높거나 같아집니다. 봇 역할을 더 위로 올려주세요.",
        ));
    }

    Ok(())
}

pub(crate) fn role_position_updates(
    bottom_to_top: &[serenity::RoleId],
    base_position: u16,
) -> Vec<(serenity::RoleId, u16)> {
    bottom_to_top
        .iter()
        .enumerate()
        .map(|(offset, role_id)| (*role_id, base_position.saturating_add(offset as u16)))
        .collect()
}

pub(crate) async fn validate_anchor_role(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
    anchor_id: serenity::RoleId,
) -> Result<(), Error> {
    let roles = guild_id.roles(&ctx.http).await?;
    let anchor_role = roles
        .get(&anchor_id)
        .ok_or_else(|| user_error("기준 역할을 찾을 수 없습니다."))?;
    let bot_member = current_bot_member(ctx, guild_id).await?;
    let bot_highest = highest_role(&roles, &bot_member.roles)
        .ok_or_else(|| user_error("봇 최고 역할을 확인할 수 없습니다."))?;

    if anchor_role.position >= bot_highest.position {
        return Err(user_error(
            "기준 역할이 봇 최고 역할보다 높거나 같습니다. 봇 역할을 더 위로 올린 뒤 다시 시도하세요.",
        ));
    }

    Ok(())
}

pub(crate) async fn reconcile_member_roles(
    ctx: &serenity::Context,
    storage: &Storage,
    guild_id: serenity::GuildId,
    user_id: serenity::UserId,
    roles: Vec<serenity::RoleId>,
) -> Result<(), Error> {
    let config = storage.guild_config(guild_id).await;
    if config.allowed_role_ids.is_empty() {
        return Ok(());
    }

    let eligible = is_eligible(&config, &roles);
    let user_state = config.users.get(&user_id.get()).cloned();

    if eligible {
        let should_restore = user_state
            .as_ref()
            .and_then(|state| state.lost_eligibility_at)
            .is_some();
        storage
            .update_guild(guild_id, |guild| {
                if let Some(state) = guild.users.get_mut(&user_id.get()) {
                    state.lost_eligibility_at = None;
                }
            })
            .await?;

        if should_restore {
            if let Err(error) = restore_user_color(ctx, storage, guild_id, user_id, false).await {
                tracing::warn!(
                    ?error,
                    user_id = user_id.get(),
                    "failed to restore user color"
                );
            }
        }
        return Ok(());
    }

    let Some(user_state) = user_state else {
        return Ok(());
    };
    if user_state.last_hex.is_none() && user_state.current_role_id.is_none() {
        return Ok(());
    }

    let now = now_unix();
    match config.loss_policy {
        LossPolicy::Keep => {
            if user_state.lost_eligibility_at.is_none() {
                storage
                    .update_guild(guild_id, |guild| {
                        guild
                            .users
                            .entry(user_id.get())
                            .or_default()
                            .lost_eligibility_at = Some(now);
                    })
                    .await?;
            }
        }
        LossPolicy::RemoveImmediate => {
            remove_user_color(ctx, storage, guild_id, user_id, true).await?;
        }
        LossPolicy::RemoveAfter { grace_days } => {
            let lost_at = user_state.lost_eligibility_at.unwrap_or(now);
            storage
                .update_guild(guild_id, |guild| {
                    guild
                        .users
                        .entry(user_id.get())
                        .or_default()
                        .lost_eligibility_at = Some(lost_at);
                })
                .await?;

            if grace_days == 0 || now >= lost_at + i64::from(grace_days) * 24 * 60 * 60 {
                remove_user_color(ctx, storage, guild_id, user_id, true).await?;
            }
        }
    }

    Ok(())
}

pub(crate) async fn process_grace_expirations(
    ctx: &serenity::Context,
    storage: &Storage,
) -> Result<(), Error> {
    for guild_id in storage.guild_ids().await {
        let config = storage.guild_config(guild_id).await;
        let LossPolicy::RemoveAfter { grace_days } = config.loss_policy else {
            continue;
        };
        if grace_days == 0 {
            continue;
        }

        let now = now_unix();
        let expired_users = config
            .users
            .iter()
            .filter_map(|(user_id, state)| {
                let lost_at = state.lost_eligibility_at?;
                (state.current_role_id.is_some()
                    && now >= lost_at + i64::from(grace_days) * 24 * 60 * 60)
                    .then_some(serenity::UserId::new(*user_id))
            })
            .collect::<Vec<_>>();

        for user_id in expired_users {
            match guild_id.member(ctx, user_id).await {
                Ok(member) if is_eligible(&config, &member.roles) => {
                    storage
                        .update_guild(guild_id, |guild| {
                            if let Some(state) = guild.users.get_mut(&user_id.get()) {
                                state.lost_eligibility_at = None;
                            }
                        })
                        .await?;
                }
                Ok(_) => {
                    remove_user_color(ctx, storage, guild_id, user_id, true).await?;
                }
                Err(error) => {
                    tracing::debug!(
                        ?error,
                        user_id = user_id.get(),
                        "member fetch failed during grace cleanup"
                    );
                }
            }
        }
    }

    Ok(())
}

async fn remove_configured_color_roles(
    ctx: &serenity::Context,
    config: &GuildConfig,
    member: &serenity::Member,
    keep: Option<serenity::RoleId>,
) -> Result<Vec<serenity::RoleId>, Error> {
    let mut ids = config
        .color_roles
        .values()
        .copied()
        .map(serenity::RoleId::new)
        .collect::<BTreeSet<_>>();
    if let Some(state) = config.users.get(&member.user.id.get()) {
        if let Some(role_id) = state.current_role_id {
            ids.insert(serenity::RoleId::new(role_id));
        }
    }

    let mut removed = Vec::new();
    for role_id in ids {
        if Some(role_id) != keep && member.roles.contains(&role_id) {
            member.remove_role(&ctx.http, role_id).await?;
            removed.push(role_id);
        }
    }

    Ok(removed)
}

async fn cleanup_unused_color_roles(
    ctx: &serenity::Context,
    storage: &Storage,
    guild_id: serenity::GuildId,
    role_ids: Vec<serenity::RoleId>,
) {
    if role_ids.is_empty() {
        return;
    }

    if let Err(error) = cleanup_unused_color_roles_inner(ctx, storage, guild_id, role_ids).await {
        tracing::warn!(?error, "failed to cleanup unused color roles");
    }
}

async fn cleanup_unused_color_roles_inner(
    ctx: &serenity::Context,
    storage: &Storage,
    guild_id: serenity::GuildId,
    role_ids: Vec<serenity::RoleId>,
) -> Result<(), Error> {
    let candidates = role_ids.into_iter().collect::<BTreeSet<_>>();
    if candidates.is_empty() {
        return Ok(());
    }

    let config = storage.guild_config(guild_id).await;
    let roles = guild_id.roles(&ctx.http).await?;
    let protected_ids = [
        config.marker_start_role_id.map(serenity::RoleId::new),
        config.marker_end_role_id.map(serenity::RoleId::new),
    ];
    let candidates = candidates
        .into_iter()
        .filter(|role_id| !protected_ids.contains(&Some(*role_id)))
        .filter(|role_id| {
            config.color_roles.values().any(|id| *id == role_id.get())
                || roles
                    .get(role_id)
                    .is_some_and(|role| is_color_role_name(&role.name))
        })
        .collect::<BTreeSet<_>>();
    if candidates.is_empty() {
        return Ok(());
    }

    let used_role_ids = used_color_roles(&config, &candidates);
    let unused_role_ids = candidates
        .difference(&used_role_ids)
        .copied()
        .collect::<Vec<_>>();
    if unused_role_ids.is_empty() {
        return Ok(());
    }

    for role_id in &unused_role_ids {
        if roles.contains_key(role_id) {
            guild_id.delete_role(&ctx.http, *role_id).await?;
        }
    }

    storage
        .update_guild(guild_id, |guild| {
            for role_id in &unused_role_ids {
                guild.color_roles.retain(|_, id| *id != role_id.get());
                for state in guild.users.values_mut() {
                    if state.current_role_id == Some(role_id.get()) {
                        state.current_role_id = None;
                    }
                }
            }
        })
        .await?;

    Ok(())
}

pub(crate) fn used_color_roles(
    config: &GuildConfig,
    role_ids: &BTreeSet<serenity::RoleId>,
) -> BTreeSet<serenity::RoleId> {
    config
        .users
        .values()
        .filter_map(|state| state.current_role_id.map(serenity::RoleId::new))
        .filter(|role_id| role_ids.contains(role_id))
        .collect()
}
