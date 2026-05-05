use crate::data::guild_repository::GuildRepository;
use crate::models::common::{Context, Error};
use crate::res::colors::{EMBED_COLOR_ERROR, EMBED_COLOR_SUCCESS, EMBED_COLOR_WARNING};
use chrono::NaiveDate;
use poise::{CreateReply, Modal};
use serenity::all::{CreateEmbed, Http};
use sqlx::PgPool;
use std::sync::Arc;

pub struct BirthSignupUsecase {
    guild_repo: GuildRepository,
}

impl BirthSignupUsecase {
    pub fn new(pool: Arc<PgPool>, http: Arc<Http>) -> anyhow::Result<Self> {
        let guild_repo = GuildRepository::new(pool, http.clone())?;
        Ok(BirthSignupUsecase { guild_repo })
    }

    pub async fn invoke(&self, poise_ctx: Context<'_>) -> anyhow::Result<(), Error> {
        let input_birth = if let Context::Application(app_ctx) = poise_ctx {
            // 先にモーダルを開いて interaction のタイムアウトを避ける
            let data = BirthSignupModal::execute(app_ctx).await?;
            match data {
                Some(data) => data.birth_input,
                None => return Ok(()),
            }
        } else {
            return Ok(());
        };

        // モーダル送信後はインタラクションが一度終了するため、
        // 以降の応答は defer してから行う必要がある。
        poise_ctx.defer_ephemeral().await?;

        let birth = NaiveDate::parse_from_str(&format!("1970/{input_birth}"), "%Y/%m/%d");
        if birth.is_err() {
            // 誕生日の入力フォーマットが無効
            poise_ctx
                .send(
                    CreateReply::default()
                        .embed(
                            CreateEmbed::new()
                                .title("🚨  誕生日が正しいフォーマットで入力されていないのだ。")
                                .color(EMBED_COLOR_ERROR), // 異常系の色
                        )
                        .ephemeral(true),
                )
                .await?;
            return Ok(());
        }

        // コマンドが実行されたギルドのギルドIDを取得
        let guild_id = self
            .guild_repo
            .fetch_guild_id_from_command(poise_ctx)
            .await?;
        let guild_id = i64::from(guild_id);
        let guild_name = poise_ctx
            .guild()
            .map(|guild| guild.name.clone())
            .unwrap_or_else(|| format!("guild-{guild_id}"));

        // コマンドを実行したメンバーのメンバーIDを取得;
        let member_id = i64::from(poise_ctx.author().id);

        // 初回参加メンバーでも登録できるよう、対象レコードを事前に作成しておく
        self.guild_repo
            .add_guild(guild_id, Some(guild_name.as_str()))
            .await?;
        self.guild_repo
            .add_member(guild_id, member_id, None)
            .await?;

        // ギルドIDとメンバーIDに一致するメンバー情報をguild_memberテーブルから取得
        let member_birth = self
            .guild_repo
            .get_member_birth(guild_id, member_id)
            .await?;

        if member_birth.is_none() {
            // メンバー情報に誕生日が存在しない
            // guild_memberテーブルのメンバーIDに一致するにメンバーの誕生日を更新
            self.guild_repo
                .update_member_birth(guild_id, member_id, birth?)
                .await?;

            // 「誕生日通知の登録が完了したこと」をメッセージで通知
            poise_ctx
                .send(
                    CreateReply::default()
                        .embed(
                            CreateEmbed::new()
                                .title("✅  誕生日の通知登録が完了したのだ。")
                                .color(EMBED_COLOR_SUCCESS), // 正常系の色
                        )
                        .content("登録した日付の正午（12:00）に誕生日が通知されるのだ。")
                        .ephemeral(true),
                )
                .await?;
        } else {
            // メンバー情報に誕生日が存在する
            // 「すでに誕生日通知が登録済であること」をメッセージで通知
            poise_ctx
                .send(
                    CreateReply::default()
                        .embed(
                            CreateEmbed::new()
                                .title("⚠️ 誕生日はすでに登録済みなのだ")
                                .color(EMBED_COLOR_WARNING), // 警告系の色
                        )
                        .ephemeral(true),
                )
                .await?;
        }

        Ok(())
    }
}

#[derive(Debug, Modal)]
#[name = "誕生日の通知登録"] // 最初のタイトル
struct BirthSignupModal {
    #[name = "自身の誕生日を入力するのだ"] // フィールドのタイトル
    #[placeholder = "02/01"]
    #[min_length = 5]
    #[max_length = 5]
    birth_input: String,
}
