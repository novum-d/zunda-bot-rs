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
        let member_ids_map_by_guild = self.guild_repo.get_all_members().await?;
        for GuildMember {
            guild_id,
            member_id,
            birth,
            last_notified,
        } in member_ids_map_by_guild
        {
            if let Some(birth) = birth {
                let guild_id = GuildId::new(u64::try_from(guild_id)?);
                let channels = guild_id.channels(http).await?;
                let now = Tokyo.from_utc_datetime(&Local::now().naive_utc());
                if let Some((_, channel)) = channels.iter()
                    .find(|(_, ch)| {
                        // ch.kind == ChannelType::Text && (ch.name == "一般" || ch.name == "general")
                        ch.kind == ChannelType::Text && ch.name == "bot"
                    } && (last_notified.is_none() || last_notified.is_some_and(|last| last.year() < now.year()))
                        && birth.month() == now.date_naive().month()
                        && birth.day() == now.day())
                {
                    // TODO: 削除
                    let member_id: i64 = 312936834264989696;
                    let guild_id: GuildId = GuildId::new(1393513606548553758);
                    let mention = format!("<@{}>", member_id);
                    let main_content = format!("@here\n今日は「🎂 {} さんのお誕生日 🎂」！\n\n今年も自分らしい１年を過ごせるとよきなのだ！！！", mention);
                    let member = guild_id.member(http, u64::try_from(member_id)?).await?;
                    let msg = channel.id.send_message(http, CreateMessage::new()
                        .content(main_content)
                        .embed(
                            CreateEmbed::new()
                                .title(member.display_name())
                                .thumbnail(member.user.avatar_url().unwrap_or_default())
                                .description(birth.format("%m/%d").to_string())
                        ),
                    ).await?;
                    msg.react(http, ReactionType::Unicode("🎉".to_string())).await?;

                    let sub_content = format!("{} さん\nお誕生日おめでとうなのだ🎉\nいつもありがとなのだ！", mention);
                    channel.id
                        .send_message(http, CreateMessage::new()
                            .content(sub_content)
                            .reference_message(&msg),
                        )
                        .await?;

                    self.guild_repo.update_last_notified(
                        i64::from(guild_id),
                        member_id,
                        now.date_naive(),
                    ).await?;
                }
            }
        }
        Ok(())
    }
}
