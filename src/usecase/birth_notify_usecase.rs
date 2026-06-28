use crate::data::guild_repository::GuildRepository;
use crate::models::common::Error;
use crate::models::data::GuildMember;
use chrono::{Datelike, Local, TimeZone};
use chrono_tz::Asia::Tokyo;
use serenity::all::{
    ChannelId, ChannelType, CreateEmbed, CreateMessage, GuildId, Http, ReactionType,
};
use sqlx::PgPool;
use std::collections::HashMap;
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
        let now = Tokyo.from_utc_datetime(&Local::now().naive_utc());
        let members = self.guild_repo.get_all_members().await?;

        let mut channel_cache: HashMap<i64, Vec<ChannelId>> = HashMap::new();

        for GuildMember {
            guild_id,
            member_id,
            birth,
            last_notified,
            ..
        } in members
        {
            let birth = match birth {
                None => continue,
                Some(birth) => birth,
            };

            let is_not_sending_enable = last_notified
                .is_some_and(|last_notified| last_notified.year() >= now.year())
                || birth.month() != now.date_naive().month()
                || birth.day() != now.day();
            if is_not_sending_enable {
                continue;
            }

            let serenity_guild_id = GuildId::new(u64::try_from(guild_id)?);

            let channel_ids = if let Some(cached) = channel_cache.get(&guild_id) {
                cached.clone()
            } else {
                let configured = self.guild_repo.get_notification_channels(guild_id).await?;
                let ids: Vec<ChannelId> = if configured.is_empty() {
                    let channels = serenity_guild_id.channels(http).await?;
                    match channels.into_values().find(|ch| {
                        ch.kind == ChannelType::Text && (ch.name == "一般" || ch.name == "general")
                    }) {
                        None => vec![],
                        Some(ch) => vec![ch.id],
                    }
                } else {
                    configured
                        .into_iter()
                        .filter_map(|id| u64::try_from(id).ok().map(ChannelId::new))
                        .collect()
                };
                channel_cache.insert(guild_id, ids.clone());
                ids
            };

            if channel_ids.is_empty() {
                continue;
            }

            let mention = format!("<@{member_id}>");
            let main_content = format!("@here\n今日は「🎂 {mention} さんのお誕生日 🎂」！\n\n今年も自分らしい１年を過ごせるとよきなのだ！！！");
            let member = serenity_guild_id
                .member(http, u64::try_from(member_id)?)
                .await?;

            for channel_id in channel_ids {
                let msg = channel_id
                    .send_message(
                        http,
                        CreateMessage::new().content(&main_content).embed(
                            CreateEmbed::new()
                                .title(member.display_name())
                                .thumbnail(member.user.avatar_url().unwrap_or_default())
                                .description(birth.format("%m/%d").to_string()),
                        ),
                    )
                    .await?;
                msg.react(http, ReactionType::Unicode("🎉".to_string()))
                    .await?;
                let sub_content =
                    format!("{mention} さん\nお誕生日おめでとうなのだ🎉\nいつもありがとなのだ！");
                channel_id
                    .send_message(
                        http,
                        CreateMessage::new()
                            .content(sub_content)
                            .reference_message(&msg),
                    )
                    .await?;
            }

            self.guild_repo
                .update_last_notified(guild_id, member_id, now.date_naive())
                .await?;
        }
        Ok(())
    }
}
