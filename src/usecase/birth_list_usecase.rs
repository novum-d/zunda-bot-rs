use crate::data::guild_repository::GuildRepository;
use crate::models::common::{Context, Error};
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
            anyhow::anyhow!(err_msg)
        })?;
        let members = self
            .guild_repo
            .get_members_by_guild_id(i64::from(guild_id))
            .await?;
        let birth_list = join_all(members.into_iter().map(|member| async move {
            let latest_member_id = u64::try_from(member.member_id).ok()?;
            let latest_member = guild_id.member(&self.http, latest_member_id).await.ok()?;
            member.birth.map(|birth| {
                format!(
                    "・{}: {}\n",
                    latest_member.display_name(),
                    birth.format("%m/%d"),
                )
            })
        }))
            .await
            .into_iter()
            .filter_map(|x| x)
            .collect::<Vec<_>>();

        if birth_list.is_empty() {
            poise_ctx
                .send(
                    CreateReply::default()
                        .embed(
                            CreateEmbed::new()
                                .title("⚠️ 誕生日が登録されていないのだ")
                                .color(0xff9900),
                        ) // オレンジ色
                        .ephemeral(true),
                )
                .await?;
        } else {
            let content = format!("# 誕生日リスト\n{}", birth_list.join(""));
            poise_ctx
                .send(CreateReply::default().content(content))
                .await?;
        }
        Ok(())
    }
}
