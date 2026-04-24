use crate::data::guild_repository::GuildRepository;
use crate::models::common::Error;
use crate::models::data::GuildMember;
use chrono::{Datelike, Local, TimeZone};
use chrono_tz::Asia::Tokyo;
use serenity::all::{ChannelType, CreateEmbed, CreateMessage, GuildId, Http, ReactionType};
use sqlx::PgPool;
use std::sync::Arc;

pub struct BirthNotifyUsecase {
    guild_repo: GuildRepository,
    http: Arc<Http>,
}

impl BirthNotifyUsecase {
    pub fn new(pool: Arc<PgPool>, http: Arc<Http>) -> anyhow::Result<Self> {
        let guild_repo = GuildRepository::new(pool, http.clone())?;
        Ok(BirthNotifyUsecase {
            guild_repo,
            http: http.clone(),
        })
    }

    pub async fn invoke(&self) -> anyhow::Result<(), Error> {
        let http = &self.http;
        let members = self.guild_repo.get_all_members().await?;
        for GuildMember {
            guild_id,
            member_id,
            birth,
            last_notified,
        } in members
        {
            // メンバーの誕生日を取得
            let birth = match birth {
                None => continue, // メンバーの誕生日が存在しない
                Some(birth) => birth,
            };

            // タイムゾーン"Asia/Tokyo"の現在日時を取得
            let now = Tokyo.from_utc_datetime(&Local::now().naive_utc());
            let is_not_sending_enable = last_notified
                .is_some_and(|last_notified| last_notified.year() >= now.year())
                || birth.month() != now.date_naive().month()
                || birth.day() != now.day();
            if is_not_sending_enable {
                continue;
            };

            // メンバーのギルドIDからチャンネル情報を取得
            let guild_id = GuildId::new(u64::try_from(guild_id)?);
            let channels = guild_id.channels(http).await?;
            let general_channel = channels.iter().find(|(_, ch)| {
                ch.kind == ChannelType::Text && (ch.name == "一般" || ch.name == "general")
            });
            let channel_id = match general_channel {
                None => continue, // "一般"または"general"のチャンネル名が存在しない
                Some((_, channel)) => channel.id,
            };

            // 誕生日のメッセージをメンバーのメンションをつけて、"一般"または"general"のチャンネルに送信
            let mention = format!("<@{member_id}>");
            let main_content = format!("@here\n今日は「🎂 {mention} さんのお誕生日 🎂」！\n\n今年も自分らしい１年を過ごせるとよきなのだ！！！");
            let member = guild_id.member(http, u64::try_from(member_id)?).await?;
            let msg = channel_id
                .send_message(
                    http,
                    CreateMessage::new().content(main_content).embed(
                        CreateEmbed::new()
                            .title(member.display_name())
                            .thumbnail(member.user.avatar_url().unwrap_or_default())
                            .description(birth.format("%m/%d").to_string()),
                    ),
                )
                .await?;

            // 誕生日のメッセージにリアクションをつける
            msg.react(http, ReactionType::Unicode("🎉".to_string()))
                .await?;

            // お祝いメッセージの一例を誕生日のメッセージのリプライとして送信
            let sub_content = format!(
                "{mention} さん\nお誕生日おめでとうなのだ🎉\nいつもありがとなのだ！"
            );
            channel_id
                .send_message(
                    http,
                    CreateMessage::new()
                        .content(sub_content)
                        .reference_message(&msg),
                )
                .await?;

            // guild_memberテーブルに誕生日を通知したメンバーの最終通知日時を記録
            self.guild_repo
                .update_last_notified(i64::from(guild_id), member_id, now.date_naive())
                .await?;
        }
        Ok(())
    }
}
