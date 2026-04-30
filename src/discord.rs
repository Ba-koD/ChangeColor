use poise::serenity_prelude as serenity;

use crate::{Context, Error, user_error};

pub(crate) async fn ensure_admin(ctx: Context<'_>) -> Result<bool, Error> {
    let Some(member) = ctx.author_member().await else {
        reply_ephemeral(ctx, "서버 안에서만 사용할 수 있습니다.").await?;
        return Ok(false);
    };

    let is_admin = member
        .permissions
        .is_some_and(|permissions| permissions.administrator());
    if !is_admin {
        reply_ephemeral(
            ctx,
            "`/컬러설정`은 Administrator 권한이 있는 서버 관리자만 사용할 수 있습니다.",
        )
        .await?;
    }

    Ok(is_admin)
}

pub(crate) fn require_guild_id(ctx: Context<'_>) -> Result<serenity::GuildId, Error> {
    ctx.guild_id()
        .ok_or_else(|| user_error("이 명령어는 서버에서만 사용할 수 있습니다."))
}

pub(crate) async fn reply_ephemeral(
    ctx: Context<'_>,
    content: impl Into<String>,
) -> Result<(), Error> {
    ctx.send(
        poise::CreateReply::default()
            .content(content.into())
            .ephemeral(true),
    )
    .await?;
    Ok(())
}

pub(crate) async fn current_bot_member(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
) -> Result<serenity::Member, Error> {
    let bot_id = ctx.cache.current_user().id;
    Ok(guild_id.member(&ctx.http, bot_id).await?)
}

pub(crate) async fn resolve_application_id(http: &serenity::Http) -> Result<u64, Error> {
    Ok(http.get_current_user().await?.id.get())
}
