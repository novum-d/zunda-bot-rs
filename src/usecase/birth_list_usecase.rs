use crate::data::guild_repository::GuildRepository;
use crate::models::common::{Context, Error};
use crate::res::colors::{EMBED_COLOR_SUCCESS, EMBED_COLOR_WARNING};
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
        // コマンドが実行されたギルドのギルドIDを取得
        let guild_id = self
            .guild_repo
            .fetch_guild_id_from_command(poise_ctx)
            .await?;

        // ギルドIDに一致するメンバー情報リストをguild_memberテーブルから取得
        let mut members = self
            .guild_repo
            .get_members_by_guild_id(i64::from(guild_id))
            .await?
            .into_iter()
            // メンバー情報リストから「誕生日が存在するもの」をフィルター
            .filter(|member| member.birth.is_some())
            .collect::<Vec<_>>();

        let reply = if members.is_empty() {
            // 「誕生日通知を登録しているメンバーがいないこと」をメッセージで通知
            CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("⚠️ 誕生日が登録されていないのだ")
                        .color(EMBED_COLOR_WARNING), // 警告系の色
                )
                .ephemeral(true)
        } else {
            // メンバー情報リストが誕生日の降順になるようにソート
            members.sort_by_key(|m| m.birth.map(|b| (b.month(), b.day())));

            // メンバーの誕生日とディスプレイ名のリストをメッセージで通知
            let birth_features = members.into_iter().map(move |member| async move {
                let latest_member_id = u64::try_from(member.member_id).ok()?;
                let latest_member = guild_id.member(&self.http, latest_member_id).await.ok()?;
                member.birth.map(|birth| {
                    format!(
                        "・{}: {}\n",
                        birth.format("%m/%d"),
                        latest_member.display_name(),
                    )
                })
            });
            let birth_list = join_all(birth_features)
                .await
                .into_iter()
                .filter_map(|x| x)
                .collect::<Vec<_>>();
            CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("🎉 誕生日リスト")
                        .description(birth_list.join(""))
                        .color(EMBED_COLOR_SUCCESS), // 正常系の色
                )
                .ephemeral(true)
        };
        poise_ctx.send(reply).await?;

        Ok(())
    }
}
