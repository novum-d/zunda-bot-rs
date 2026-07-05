use crate::data::guild_repository::GuildRepository;
use crate::models::common::{Context, Error};
use crate::res::colors::{EMBED_COLOR_SUCCESS, EMBED_COLOR_WARNING};
use chrono::Datelike;
use poise::futures_util::future::join_all;
use poise::CreateReply;
use serenity::all::{CreateEmbed, GuildId, Http};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
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

        let view = self.build_view(guild_id).await?;
        poise_ctx.send(view.into_reply()).await?;

        Ok(())
    }

    pub async fn build_view(&self, guild_id: GuildId) -> anyhow::Result<BirthListView> {
        // ギルドIDに一致するメンバー情報リストをguild_memberテーブルから取得
        let mut members = self
            .guild_repo
            .get_members_by_guild_id(i64::from(guild_id))
            .await?
            .into_iter()
            // メンバー情報リストから「誕生日が存在するもの」をフィルター
            .filter(|member| member.birth.is_some())
            .collect::<Vec<_>>();

        if members.is_empty() {
            // 「誕生日通知を登録しているメンバーがいないこと」をメッセージで通知
            Ok(BirthListView::Empty)
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
                .flatten()
                .collect::<Vec<_>>();
            Ok(BirthListView::List {
                description: birth_list.join(""),
            })
        }
    }
}

pub enum BirthListView {
    Empty,
    List { description: String },
}

impl BirthListView {
    pub fn into_reply(self) -> CreateReply {
        match self {
            Self::Empty => CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("⚠️ 誕生日が登録されていないのだ")
                        .color(EMBED_COLOR_WARNING),
                )
                .ephemeral(true),
            Self::List { description } => CreateReply::default()
                .embed(
                    CreateEmbed::new()
                        .title("🎉 誕生日リスト")
                        .description(description)
                        .color(EMBED_COLOR_SUCCESS),
                )
                .ephemeral(true),
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Empty => "⚠️ 誕生日が登録されていないのだ",
            Self::List { .. } => "🎉 誕生日リスト",
        }
    }

    pub fn color(&self) -> u32 {
        match self {
            Self::Empty => EMBED_COLOR_WARNING,
            Self::List { .. } => EMBED_COLOR_SUCCESS,
        }
    }

    pub fn description(&self) -> Option<&str> {
        match self {
            Self::Empty => None,
            Self::List { description } => Some(description),
        }
    }
}
