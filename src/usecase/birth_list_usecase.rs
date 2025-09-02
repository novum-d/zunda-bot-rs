use crate::data::guild_repository::GuildRepository;
use crate::models::common::{Context, Error};
use chrono::Datelike;
use poise::futures_util::future::join_all;
use poise::CreateReply;
use serenity::all::{CreateEmbed, Http};
use sqlx::PgPool;
use std::sync::Arc;

pub struct BirthListUsecase {
    guild_repo: GuildRepository,
    http: Arc<Http>,
}

impl BirthListUsecase {
    pub fn new(pool: Arc<PgPool>, http: Arc<Http>) -> anyhow::Result<Self> {
        let guild_repo = GuildRepository::new(pool, http.clone())?;
        Ok(BirthListUsecase {
            guild_repo,
            http: http.clone(),
        })
    }

    pub async fn invoke(&self, poise_ctx: Context<'_>) -> anyhow::Result<(), Error> {
        let guild_id = poise_ctx.guild_id().ok_or_else(|| {
            let err_msg = "Could not retrieve the Guild ID.";
            tracing::error!(err_msg);
        }).unwrap_or_default();
        let mut members = self
            .guild_repo
            .get_members_by_guild_id(i64::from(guild_id))
            .await?;
        members.sort_by_key(|m| m.birth.map(|b| (b.month(), b.day())));
        let birth_features = members
            .into_iter()
            .filter(|member| member.birth.is_some())
            .map(move |member| {
                async move {
                    let latest_member_id = u64::try_from(member.member_id).ok()?;
                    let latest_member = guild_id.member(&self.http, latest_member_id).await.ok()?;
                    member.birth.map(|birth| {
                        format!(
                            "・{}: {}\n",
                            birth.format("%m/%d"),
                            latest_member.display_name(),
                        )
                    })
                }
            });
        let birth_list = join_all(birth_features)
            .await
            .into_iter()
            .filter_map(|x| x)
            .collect::<Vec<_>>();

        let reply = if birth_list.is_empty() {
            CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("⚠️ 誕生日が登録されていないのだ")
                        .color(0xffd700), // 警告系の色
                )
                .ephemeral(true)
        } else {
            CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("🎉 誕生日リスト")
                        .description(birth_list.join(""))
                        .color(0x00ff00), // 正常系の色
                )
                .ephemeral(true)
        };

        poise_ctx
            .send(reply)
            .await?;

        Ok(())
    }
}
