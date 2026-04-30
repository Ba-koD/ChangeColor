use poise::serenity_prelude as serenity;

use crate::config::MAX_GRACE_DAYS;
use crate::discord::{current_bot_member, ensure_admin, reply_ephemeral, require_guild_id};
use crate::roles::{
    apply_color_for_user, ensure_markers, remove_user_color, reposition_color_block,
    restore_user_color, validate_anchor_role,
};
use crate::storage::LossPolicy;
use crate::util::{format_policy, highest_role, is_managed_color_role, mention_role};
use crate::{Context, Data, Error, user_error};

#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
enum LossMode {
    #[name = "유지"]
    Keep,
    #[name = "즉시제거"]
    RemoveImmediate,
    #[name = "유예제거"]
    RemoveAfter,
}

#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
enum ColorAction {
    #[name = "제거"]
    Remove,
    #[name = "복구"]
    Restore,
}

/// 컬러 봇 설정
#[poise::command(
    slash_command,
    guild_only,
    rename = "컬러설정",
    subcommands(
        "config_allow_role_add",
        "config_allow_role_remove",
        "config_policy",
        "config_anchor",
        "config_reorder",
        "config_status"
    ),
    default_member_permissions = "ADMINISTRATOR",
    required_bot_permissions = "MANAGE_ROLES"
)]
async fn color_config(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// 사용 가능한 역할 추가
#[poise::command(slash_command, guild_only, rename = "허용역할추가")]
async fn config_allow_role_add(
    ctx: Context<'_>,
    #[description = "컬러 명령어를 사용할 수 있는 역할"] role: serenity::Role,
) -> Result<(), Error> {
    if !ensure_admin(ctx).await? {
        return Ok(());
    }

    let guild_id = require_guild_id(ctx)?;
    ctx.data()
        .storage
        .update_guild(guild_id, |guild| {
            guild.allowed_role_ids.insert(role.id.get());
        })
        .await?;

    reply_ephemeral(
        ctx,
        format!("사용 가능 역할에 {}을 추가했습니다.", mention_role(role.id)),
    )
    .await
}

/// 사용 가능한 역할 제거
#[poise::command(slash_command, guild_only, rename = "허용역할제거")]
async fn config_allow_role_remove(
    ctx: Context<'_>,
    #[description = "컬러 명령어 사용 권한에서 제거할 역할"] role: serenity::Role,
) -> Result<(), Error> {
    if !ensure_admin(ctx).await? {
        return Ok(());
    }

    let guild_id = require_guild_id(ctx)?;
    ctx.data()
        .storage
        .update_guild(guild_id, |guild| {
            guild.allowed_role_ids.remove(&role.id.get());
        })
        .await?;

    reply_ephemeral(
        ctx,
        format!(
            "사용 가능 역할에서 {}을 제거했습니다.",
            mention_role(role.id)
        ),
    )
    .await
}

/// 권한 상실 시 컬러 처리 정책 설정
#[poise::command(slash_command, guild_only, rename = "정책")]
async fn config_policy(
    ctx: Context<'_>,
    #[description = "권한 상실 시 처리 방식"] mode: LossMode,
    #[description = "유예제거일 때 0일부터 7일까지"] grace_days: Option<i64>,
) -> Result<(), Error> {
    if !ensure_admin(ctx).await? {
        return Ok(());
    }

    let policy = match mode {
        LossMode::Keep => LossPolicy::Keep,
        LossMode::RemoveImmediate => LossPolicy::RemoveImmediate,
        LossMode::RemoveAfter => {
            let days = grace_days.unwrap_or(0);
            if !(0..=i64::from(MAX_GRACE_DAYS)).contains(&days) {
                reply_ephemeral(ctx, "유예기간은 0일부터 7일까지 설정할 수 있습니다.").await?;
                return Ok(());
            }
            LossPolicy::RemoveAfter {
                grace_days: days as u8,
            }
        }
    };

    let guild_id = require_guild_id(ctx)?;
    ctx.data()
        .storage
        .update_guild(guild_id, |guild| {
            guild.loss_policy = policy.clone();
        })
        .await?;

    reply_ephemeral(
        ctx,
        format!(
            "권한 상실 정책을 `{}`로 변경했습니다.",
            format_policy(&policy)
        ),
    )
    .await
}

/// 컬러 역할 묶음을 이 역할 바로 위에 배치
#[poise::command(slash_command, guild_only, rename = "위치기준")]
async fn config_anchor(
    ctx: Context<'_>,
    #[description = "컬러 역할 묶음이 바로 위에 놓일 기준 역할"] role: serenity::Role,
) -> Result<(), Error> {
    if !ensure_admin(ctx).await? {
        return Ok(());
    }

    let guild_id = require_guild_id(ctx)?;
    let storage = ctx.data().storage.clone();
    ctx.defer_ephemeral().await?;
    let config = storage.guild_config(guild_id).await;
    if is_managed_color_role(&config, role.id) {
        reply_ephemeral(
            ctx,
            "marker 역할이나 컬러 역할 자체는 위치기준으로 지정할 수 없습니다.",
        )
        .await?;
        return Ok(());
    }

    let result: Result<String, Error> = async {
        validate_anchor_role(ctx.serenity_context(), guild_id, role.id).await?;

        storage
            .update_guild(guild_id, |guild| {
                guild.anchor_role_id = Some(role.id.get());
            })
            .await?;

        ensure_markers(ctx.serenity_context(), &storage, guild_id).await?;
        let summary = reposition_color_block(ctx.serenity_context(), &storage, guild_id).await?;
        Ok(format!(
            "컬러 역할 묶음 위치기준을 {} 바로 위로 설정했습니다.\n{}",
            mention_role(role.id),
            summary
        ))
    }
    .await;

    match result {
        Ok(message) => reply_ephemeral(ctx, message).await,
        Err(error) => reply_ephemeral(ctx, format!("실패: {error}")).await,
    }
}

/// 저장된 기준 역할 바로 위로 컬러 역할 묶음 재정렬
#[poise::command(slash_command, guild_only, rename = "재정렬")]
async fn config_reorder(ctx: Context<'_>) -> Result<(), Error> {
    if !ensure_admin(ctx).await? {
        return Ok(());
    }

    let guild_id = require_guild_id(ctx)?;
    let storage = ctx.data().storage.clone();
    ctx.defer_ephemeral().await?;

    let result: Result<String, Error> = async {
        ensure_markers(ctx.serenity_context(), &storage, guild_id).await?;
        reposition_color_block(ctx.serenity_context(), &storage, guild_id).await
    }
    .await;

    match result {
        Ok(message) => reply_ephemeral(ctx, message).await,
        Err(error) => reply_ephemeral(ctx, format!("실패: {error}")).await,
    }
}

/// 현재 컬러 봇 설정 확인
#[poise::command(slash_command, guild_only, rename = "상태")]
async fn config_status(ctx: Context<'_>) -> Result<(), Error> {
    if !ensure_admin(ctx).await? {
        return Ok(());
    }

    let guild_id = require_guild_id(ctx)?;
    let storage = ctx.data().storage.clone();
    ctx.defer_ephemeral().await?;
    let config = storage.guild_config(guild_id).await;
    let roles = guild_id.roles(ctx.http()).await?;
    let bot_member = current_bot_member(ctx.serenity_context(), guild_id).await?;
    let bot_highest = highest_role(&roles, &bot_member.roles);

    let allowed_roles = if config.allowed_role_ids.is_empty() {
        "없음".to_string()
    } else {
        config
            .allowed_role_ids
            .iter()
            .map(|id| mention_role(serenity::RoleId::new(*id)))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let anchor = config
        .anchor_role_id
        .map(|id| {
            if roles.contains_key(&serenity::RoleId::new(id)) {
                mention_role(serenity::RoleId::new(id))
            } else {
                format!("삭제됨({id})")
            }
        })
        .unwrap_or_else(|| "없음".to_string());

    let mut warnings = Vec::new();
    if config.allowed_role_ids.is_empty() {
        warnings.push("사용 가능 역할이 없어 `/컬러`는 아무도 사용할 수 없습니다.".to_string());
    }
    if config.anchor_role_id.is_none() {
        warnings.push(
            "위치기준이 없어 컬러 역할이 자동으로 원하는 위치에 정렬되지 않습니다.".to_string(),
        );
    }
    if let (Some(bot_role), Some(anchor_id)) = (bot_highest, config.anchor_role_id) {
        if roles
            .get(&serenity::RoleId::new(anchor_id))
            .is_some_and(|anchor_role| anchor_role.position >= bot_role.position)
        {
            warnings.push(
                "위치기준 역할이 봇 최고 역할보다 높거나 같아 재정렬할 수 없습니다.".to_string(),
            );
        }
    }

    let warning_text = warnings
        .into_iter()
        .map(|warning| format!("- {warning}"))
        .collect::<Vec<_>>()
        .join("\n");

    let bot_role_text = bot_highest
        .map(|role| format!("{} position {}", mention_role(role.id), role.position))
        .unwrap_or_else(|| "확인 불가".to_string());

    let mut message = format!(
        "사용 가능 역할: {allowed_roles}\n정책: `{}`\n위치기준: {anchor}\n컬러 역할 수: {}\n기록된 유저 수: {}\n봇 최고 역할: {bot_role_text}",
        format_policy(&config.loss_policy),
        config.color_roles.len(),
        config.users.len(),
    );
    if !warning_text.is_empty() {
        message.push_str("\n\n");
        message.push_str(&warning_text);
    }
    reply_ephemeral(ctx, message).await
}

/// 내 닉네임 컬러 변경, 제거, 복구
#[poise::command(
    slash_command,
    guild_only,
    rename = "컬러",
    ephemeral,
    required_bot_permissions = "MANAGE_ROLES"
)]
async fn color(
    ctx: Context<'_>,
    #[description = "적용할 HEX 색상. 예: #ff66aa"] hex: Option<String>,
    #[description = "색상 제거 또는 마지막 색상 복구"]
    #[rename = "작업"]
    action: Option<ColorAction>,
) -> Result<(), Error> {
    let guild_id = require_guild_id(ctx)?;
    let storage = ctx.data().storage.clone();
    ctx.defer_ephemeral().await?;

    let result = match (hex, action) {
        (Some(hex), None) => apply_color_for_user(
            ctx.serenity_context(),
            &storage,
            guild_id,
            ctx.author().id,
            &hex,
            true,
        )
        .await
        .map(|hex| format!("내 컬러를 `{hex}`로 변경했습니다.")),
        (None, Some(ColorAction::Remove)) => remove_user_color(
            ctx.serenity_context(),
            &storage,
            guild_id,
            ctx.author().id,
            false,
        )
        .await
        .map(|removed| {
            if removed {
                "현재 컬러 역할을 제거했습니다.".to_string()
            } else {
                "제거할 컬러 역할이 없습니다.".to_string()
            }
        }),
        (None, Some(ColorAction::Restore)) => restore_user_color(
            ctx.serenity_context(),
            &storage,
            guild_id,
            ctx.author().id,
            true,
        )
        .await
        .map(|hex| format!("마지막 컬러 `{hex}`를 복구했습니다.")),
        (Some(_), Some(_)) => Err(user_error(
            "색상 변경과 작업 옵션은 동시에 사용할 수 없습니다.",
        )),
        (None, None) => Err(user_error(
            "색상을 바꾸려면 `hex`를 입력하고, 제거/복구는 `작업`을 선택하세요.",
        )),
    };

    match result {
        Ok(message) => reply_ephemeral(ctx, message).await,
        Err(error) => reply_ephemeral(ctx, format!("실패: {error}")).await,
    }
}

pub(crate) fn bot_commands() -> Vec<poise::Command<Data, Error>> {
    vec![color_config(), color()]
}
