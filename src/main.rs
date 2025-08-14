mod commands;
mod constants;

use crate::commands::birth::birth;
use anyhow::Context as _;
use commands::hello::hello;
use poise::futures_util::future::join_all;
use poise::serenity_prelude as serenity;
use serenity::model::gateway::GatewayIntents;
use serenity::model::id::GuildId;
use serenity::prelude::*;
use shuttle_runtime::SecretStore;
use sqlx::types::chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};

/// `Data`構造体は、Botコマンド実行時に毎回アクセスできる「ユーザーデータ」を格納するための型
/// この型にフィールドを追加することで、コマンド間で共有したい情報（設定値や状態など）を保持できる
/// `poise`フレームワークでは、各コマンドの`Context`からこの`Data`にアクセスできる
struct Data {} // User data, which is stored and accessible in all command invocations
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;


#[shuttle_runtime::main]
async fn main(
    #[shuttle_runtime::Secrets] secrets: SecretStore,
    #[shuttle_shared_db::Postgres()] pool: PgPool, // local_uriを指定するとエラーになるので記述しない
) -> shuttle_serenity::ShuttleSerenity {
    sqlx::migrate!("db/migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let token = secrets
        .get("DISCORD_TOKEN")
        .context("'DISCORD_TOKEN' was not found")?;

    let intents =
        GatewayIntents::GUILD_MEMBERS // ギルドメンバー情報取得権限
            | GatewayIntents::GUILD_MESSAGES // ギルド内のメッセージイベント受信権限
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT;

    // コマンドを作成
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                // コマンドはここに追加
                hello(),
                birth(),
            ],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                // --- ギルド情報取得  ------------------------------------------------------------------------
                // guildテーブルから「ギルドID」のリストを取得
                let local_guild_ids = select_guild_ids(&pool).await?;

                // APIから「ギルドIDとギルド名、メンバー情報リスト」の一覧を取得
                let latest_guild_ids = fetch_my_guild_ids(&ctx).await?;
                let latest_guild_futures =
                    latest_guild_ids
                        .iter()
                        .map(|guild_id| fetch_my_guild(&ctx, guild_id));
                let latest_my_guilds: Vec<MyGuild> =
                    join_all(latest_guild_futures)
                        .await
                        .into_iter()
                        .filter_map(Result::ok)
                        .collect();
                // -----------------------------------------------------------------------------------------------------


                // --- ギルド情報更新  ------------------------------------------------------------------------
                // guildテーブルから取得したギルドIDのリストに
                // 「APIで取得したギルドIDが存在するか」一つずつ検索
                let local_guild_id_set: HashSet<u64> = local_guild_ids
                    .iter()
                    .cloned()
                    .collect();
                let latest_guild_id_set: HashSet<u64> = latest_my_guilds
                    .iter()
                    .map(|my_guild| &my_guild.id)
                    .cloned()
                    .collect();

                // APIとテーブルのギルドIDが一致
                for &id in local_guild_id_set.intersection(&latest_guild_id_set) {
                    // 該当するギルドIDを持つAPIのギルド情報をguildテーブルの行へ更新
                    let my_guild =
                        latest_my_guilds
                            .iter()
                            .find(|my_guild| my_guild.id == id);
                    if let Some(my_guild) = my_guild {
                        update_guild_by_id(
                            &pool,
                            my_guild.id,
                            &my_guild.name,
                        ).await?;
                    }
                }

                // guildテーブルに存在し、APIにないギルドIDが存在
                for &id in local_guild_id_set.difference(&latest_guild_id_set) {
                    // 該当するギルドIDをguildテーブルから削除
                    delete_guild_by_id(
                        &pool,
                        id,
                    ).await?;
                }

                // APIに存在し、guildテーブルにないギルドIDが存在
                for &id in latest_guild_id_set.difference(&local_guild_id_set) {
                    // 該当するギルドIDを持つAPIをguildテーブルの行に追加
                    let my_guild = latest_my_guilds
                        .iter()
                        .find(|my_guild| my_guild.id == id);
                    insert_guild_by_id(
                        &pool,
                        id,
                        my_guild.map(|g| g.name.as_str()),
                    ).await?;
                }
                // -----------------------------------------------------------------------------------------------------


                // --- ギルドメンバー情報更新  ------------------------------------------------------------------------
                // 更新したguildテーブルから「ギルドID」のリストを再取得
                let guild_ids = select_guild_ids(&pool).await?;

                // guild_memberテーブルからギルドIDごとの「ギルドID, メンバーIDのリスト」のマップを取得
                let member_ids_map_by_guild = select_member_ids_by_guild(&pool).await?;

                // guild_memberテーブルから取得したギルドIDごとのメンバーIDのリストに
                // 「APIで取得したメンバーIDが存在するか」一つずつ検索
                for MyGuild { id, name: _, members } in &latest_my_guilds {
                    let latest_guild_id = id.clone();
                    let latest_member_ids: Vec<u64> =
                        members
                            .iter()
                            .map(|member| member.member_id)
                            .collect();
                    let latest_guild_members_set_by_guild: HashSet<&MyGuildMember> =
                        latest_my_guilds
                            .iter()
                            .filter(|my_guild| my_guild.id == latest_guild_id)
                            .flat_map(|my_guild| my_guild.members.iter())
                            .collect();

                    let local_member_id_set: HashSet<u64> =
                        member_ids_map_by_guild
                            .get(&latest_guild_id)
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .collect();

                    let latest_member_id_set: HashSet<u64> =
                        latest_guild_members_set_by_guild
                            .iter()
                            .map(|member| member.member_id)
                            .collect();

                    // APIとテーブルのメンバーIDが一致
                    for &id in local_member_id_set.intersection(&latest_member_id_set) {
                        // 該当するメンバーIDを持つAPIのメンバー情報をguild_memberテーブルの行へ更新
                        let member =
                            latest_guild_members_set_by_guild
                                .iter()
                                .find(|member| member.member_id == id);
                        if let Some(member) = member {
                            update_guild_member(
                                &pool,
                                latest_guild_id,
                                member.member_id,
                                member.nickname.as_deref(),
                            ).await?;
                        }
                    }

                    // guild_memberテーブルに存在し、APIにないメンバーIDが存在
                    for &id in local_member_id_set.difference(&latest_member_id_set) {
                        // 該当するメンバーIDをguild_memberテーブルから削除
                        delete_guild_member(
                            &pool,
                            latest_guild_id,
                            id,
                        ).await?;
                    }


                    // APIに存在し、guild_memberテーブルにないメンバーIDが存在
                    for &id in latest_member_id_set.difference(&local_member_id_set) {
                        // 該当するメンバーIDを持つAPIをguild_memberテーブルの行に追加
                        let member = latest_guild_members_set_by_guild
                            .iter()
                            .find(|member| member.member_id == id);
                        if let Some(member) = member {
                            insert_guild_member(
                                &pool,
                                latest_guild_id,
                                member.member_id,
                                member.nickname.as_deref(),
                                member.birth.map(|date| date.naive_utc()),
                            ).await?;
                        }
                    }
                }
                // -----------------------------------------------------------------------------------------------------

                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .build();

    let client = Client::builder(&token, intents)
        .framework(framework)
        .await
        .map_err(shuttle_runtime::CustomError::new)?;

    Ok(client.into())
}

#[derive(Debug)]
struct MyGuild {
    id: u64,
    name: String,
    members: Vec<MyGuildMember>,
}

#[derive(Debug, Hash, Eq, PartialEq)]
struct MyGuildMember {
    member_id: u64,
    nickname: Option<String>,
    birth: Option<DateTime<FixedOffset>>,
}

/// ギルドの情報を取得する関数
async fn fetch_my_guild(
    ctx: &serenity::Context,
    guild_id: &GuildId,
) -> anyhow::Result<MyGuild> {
    let partial_guild = ctx.http.get_guild(guild_id.clone()).await?;
    let members = partial_guild.members(
        &ctx.http,
        None,
        None,
    ).await?
        .into_iter()
        .filter_map(|member| {
            Some(MyGuildMember {
                member_id: u64::from(member.user.id),
                nickname: member.nick,
                birth: None, // 一旦、仮でepochを設定（後にDBの情報で更新）
            })
        })
        .collect::<Vec<MyGuildMember>>();

    Ok(MyGuild {
        id: u64::from(partial_guild.id),
        name: partial_guild.name,
        members,
    })
}


/// ボットが所属するギルドIDのリストを取得する関数
async fn fetch_my_guild_ids(ctx: &serenity::Context) -> anyhow::Result<Vec<GuildId>> {
    let guilds = ctx.http.get_guilds(None, None).await?;
    Ok(guilds.into_iter().map(|g| g.id).collect())
}


#[derive(Debug, sqlx::FromRow)]
struct GuildMember {
    guild_id: String,
    member_id: String,
}


/// DBからギルドIDのリストを取得するメソッド
async fn select_guild_ids(pool: &PgPool) -> anyhow::Result<Vec<u64>> {
    let guild_ids = sqlx::query_scalar!(
        r#"
        SELECT guild_id::BIGINT FROM guild
        "#
    )
        .fetch_all(pool)
        .await?
        .into_iter()
        .filter_map(|guild_id: i64| Some(u64::try_from(guild_id)))
        .collect::<Result<Vec<u64>, _>>()?;
    Ok(guild_ids)
}

/// DBのギルド情報を更新するメソッド
async fn update_guild_by_id(
    pool: &PgPool,
    guild_id: u64,
    guild_name: &str,
) -> anyhow::Result<()> {
    let guild_id: i64 = guild_id.try_into()?;

    sqlx::query!(
        r#"
        UPDATE guild
        SET name = $1
        WHERE guild_id = $2
        "#,
        guild_name,
        guild_id,
    )
        .execute(pool)
        .await?;

    Ok(())
}


/// DBのギルド情報を削除するメソッド
async fn delete_guild_by_id(
    pool: &PgPool,
    guild_id: u64,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        DELETE FROM guild_member
        WHERE guild_id = $1
        "#,
        i64::try_from(guild_id)?,
    )
        .execute(pool)
        .await?;

    sqlx::query!(
        r#"
        DELETE FROM guild
        WHERE guild_id = $1
        "#,
        i64::try_from(guild_id)?,
    )
        .execute(pool)
        .await?;
    Ok(())
}

async fn insert_guild_by_id(
    pool: &PgPool,
    guild_id: u64,
    guild_name: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO guild (guild_id, name)
        VALUES ($1, $2)
        ON CONFLICT (guild_id) DO NOTHING
        "#,
        i64::try_from(guild_id)?,
        guild_name,
    )
        .execute(pool)
        .await?;
    Ok(())
}


async fn select_member_ids_by_guild(pool: &PgPool) -> anyhow::Result<HashMap<u64, Vec<u64>>> {
    let rows = sqlx::query!(
        r#"
        SELECT guild_id::BIGINT, member_id::BIGINT FROM guild_member
        "#
    )
        .fetch_all(pool)
        .await?;

    let mut map: HashMap<u64, Vec<u64>> = HashMap::new();
    for row in rows {
        let guild_id = u64::try_from(row.guild_id)?;
        let member_id = u64::try_from(row.member_id)?;
        map.entry(guild_id).or_default().push(member_id);
    }
    Ok(map)
}


async fn update_guild_member(
    pool: &PgPool,
    guild_id: u64,
    member_id: u64,
    nickname: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        UPDATE guild_member
        SET nickname = $1
        WHERE guild_id = $2 AND member_id = $3
        "#,
        nickname,
        i64::try_from(guild_id)?,
        i64::try_from(member_id)?,
    )
        .execute(pool)
        .await?;
    Ok(())
}


async fn delete_guild_member(
    pool: &PgPool,
    guild_id: u64,
    member_id: u64,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        DELETE FROM guild_member
        WHERE guild_id = $1 AND member_id = $2
        "#,
        i64::try_from(guild_id)?,
        i64::try_from(member_id)?,
    )
        .execute(pool)
        .await?;
    Ok(())
}

async fn insert_guild_member(
    pool: &PgPool,
    guild_id: u64,
    member_id: u64,
    nickname: Option<&str>,
    birth: Option<NaiveDateTime>,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO guild_member (guild_id, member_id, nickname, birth)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (guild_id, member_id) DO NOTHING
        "#,
        i64::try_from(guild_id)?,
        i64::try_from(member_id)?,
        nickname,
        birth,
    )
        .execute(pool)
        .await?;
    Ok(())
}