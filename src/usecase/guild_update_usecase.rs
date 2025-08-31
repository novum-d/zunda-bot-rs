use crate::data::guild_repository::GuildRepository;
use crate::models::common::Error;
use crate::models::data::GuildMember;
use crate::models::domain::{MyGuild, MyGuildMember};
use poise::futures_util::future::join_all;
use serenity::all::Http;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct GuildUpdateUsecase {
    guild_repo: GuildRepository,
}

impl GuildUpdateUsecase {
    pub fn new(pool: Arc<PgPool>, http: Arc<Http>) -> anyhow::Result<Self> {
        let guild_repo = GuildRepository::new(pool, http)?;
        Ok(GuildUpdateUsecase { guild_repo })
    }

    pub async fn invoke(&self) -> anyhow::Result<(), Error> {
        // --- ギルド情報取得  ------------------------------------------------------------------------
        // guildテーブルから「ギルドID」のリストを取得
        let local_guild_ids = self.guild_repo.get_guild_ids().await?;

        // APIから「ギルドIDとギルド名、メンバー情報リスト」の一覧を取得
        let latest_guild_ids = self.guild_repo.fetch_my_guild_ids().await?;

        let latest_guild_futures = latest_guild_ids
            .iter()
            .map(|guild_id| self.guild_repo.fetch_my_guild(guild_id));
        let latest_my_guilds: Vec<MyGuild> = join_all(latest_guild_futures)
            .await
            .into_iter()
            .filter_map(Result::ok)
            .collect();
        // -----------------------------------------------------------------------------------------------------

        // --- ギルド情報更新  ------------------------------------------------------------------------
        // guildテーブルから取得したギルドIDのリストに
        // 「APIで取得したギルドIDが存在するか」一つずつ検索
        let local_guild_id_set: HashSet<i64> = local_guild_ids.iter().cloned().collect();
        let latest_guild_id_set: HashSet<i64> = latest_my_guilds
            .iter()
            .map(|my_guild| &my_guild.id)
            .cloned()
            .collect();

        // APIとテーブルのギルドIDが一致
        for &id in local_guild_id_set.intersection(&latest_guild_id_set) {
            // 該当するギルドIDを持つAPIのギルド情報をguildテーブルの行へ更新
            let my_guild = latest_my_guilds.iter().find(|my_guild| my_guild.id == id);
            if let Some(my_guild) = my_guild {
                self.guild_repo
                    .update_guild(my_guild.id, &my_guild.name)
                    .await?;
            }
        }

        // guildテーブルに存在し、APIにないギルドIDが存在
        for &id in local_guild_id_set.difference(&latest_guild_id_set) {
            // 該当するギルドIDをguildテーブルから削除
            self.guild_repo.delete_guild(id).await?;
        }

        // APIに存在し、guildテーブルにないギルドIDが存在
        for &id in latest_guild_id_set.difference(&local_guild_id_set) {
            // 該当するギルドIDを持つAPIをguildテーブルの行に追加
            let my_guild = latest_my_guilds.iter().find(|my_guild| my_guild.id == id);
            self.guild_repo
                .add_guild(id, my_guild.map(|g| g.name.as_str()))
                .await?;
        }
        // -----------------------------------------------------------------------------------------------------

        // --- ギルドメンバー情報更新  ------------------------------------------------------------------------
        // guild_memberテーブルからギルドIDごとの「ギルドID, メンバーIDのリスト」のマップを取得
        let rows = self.guild_repo.get_all_members().await?;
        let mut member_ids_map_by_guild: HashMap<i64, Vec<i64>> = HashMap::new();
        for GuildMember {
            guild_id,
            member_id,
            birth: _,
            last_notified: _,
        } in rows
        {
            member_ids_map_by_guild
                .entry(guild_id)
                .or_default()
                .push(member_id);
        }

        // guild_memberテーブルから取得したギルドIDごとのメンバーIDのリストに
        // 「APIで取得したメンバーIDが存在するか」一つずつ検索
        for MyGuild {
            id,
            name: _,
            members: _,
        } in &latest_my_guilds
        {
            let latest_guild_id = id.clone();
            let latest_guild_members_set_by_guild: HashSet<&MyGuildMember> = latest_my_guilds
                .iter()
                .filter(|my_guild| my_guild.id == latest_guild_id)
                .flat_map(|my_guild| my_guild.members.iter())
                .collect();

            let local_member_id_set: HashSet<i64> = member_ids_map_by_guild
                .get(&latest_guild_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect();

            let latest_member_id_set: HashSet<i64> = latest_guild_members_set_by_guild
                .iter()
                .map(|member| member.member_id)
                .collect();

            // APIとテーブルのメンバーIDが一致
            for &id in local_member_id_set.intersection(&latest_member_id_set) {
                // 該当するメンバーIDを持つAPIのメンバー情報をguild_memberテーブルの行へ更新
                let member = latest_guild_members_set_by_guild
                    .iter()
                    .find(|member| member.member_id == id);
                if let Some(_) = member {
                    // メンバー情報を更新する内容があれば、ここに処理を置く
                }
            }

            // guild_memberテーブルに存在し、APIにないメンバーIDが存在
            for &id in local_member_id_set.difference(&latest_member_id_set) {
                // 該当するメンバーIDをguild_memberテーブルから削除
                self.guild_repo.delete_member(latest_guild_id, id).await?;
            }

            // APIに存在し、guild_memberテーブルにないメンバーIDが存在
            for &id in latest_member_id_set.difference(&local_member_id_set) {
                // 該当するメンバーIDを持つAPIをguild_memberテーブルの行に追加
                let member = latest_guild_members_set_by_guild
                    .iter()
                    .find(|member| member.member_id == id);
                if let Some(member) = member {
                    self.guild_repo
                        .add_member(latest_guild_id, member.member_id, member.birth)
                        .await?;
                }
            }
        }
        // -----------------------------------------------------------------------------------------------------

        Ok(())
    }
}
