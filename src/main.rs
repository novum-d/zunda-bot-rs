mod commands;
mod constants;

use crate::commands::birth::birth;
use crate::commands::test::test;
use anyhow::Context as _;
use chrono::Datelike;
use chrono_tz::Asia::Tokyo;
use commands::hello::hello;
use poise::futures_util::future::join_all;
use poise::serenity_prelude as serenity;
use serenity::all::{ChannelType, CreateEmbed, CreateMessage, Http, ReactionType};
use serenity::model::gateway::GatewayIntents;
use serenity::model::id::GuildId;
use serenity::prelude::*;
use shuttle_runtime::SecretStore;
use sqlx::types::chrono::{Local, NaiveDate, NaiveTime, TimeZone};
use sqlx::{FromRow, PgPool};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// `Data`構造体は、Botコマンド実行時に毎回アクセスできる「ユーザーデータ」を格納するための型
/// この型にフィールドを追加することで、コマンド間で共有したい情報（設定値や状態など）を保持できる
/// `poise`フレームワークでは、各コマンドの`Context`からこの`Data`にアクセスできる
pub struct Data {
    pub pool: PgPool,
} // User data, which is stored and accessible in all command invocations
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
                test(), // デバッグ用
            ],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                let http = ctx.http();

                update_guilds(&pool, http).await?;

                // -----------------------------------------------------------------------------------------------------

                tokio::spawn(birthday_cron_worker(pool.clone(), Arc::clone(&ctx.http)));

                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                let data = Data { pool };
                Ok(data)
            })
        })
        .build();

    let client = Client::builder(&token, intents)
        .framework(framework)
        .await
        .map_err(shuttle_runtime::CustomError::new)?;

    Ok(client.into())
}

pub async fn update_guilds(
    pool: &PgPool,
    http: &Http,
) -> anyhow::Result<()> {
    // --- ギルド情報取得  ------------------------------------------------------------------------
    // guildテーブルから「ギルドID」のリストを取得
    let local_guild_ids = select_guild_ids(&pool).await?;

    // APIから「ギルドIDとギルド名、メンバー情報リスト」の一覧を取得
    let latest_guild_ids = fetch_my_guild_ids(http).await?;
    let latest_guild_futures =
        latest_guild_ids
            .iter()
            .map(|guild_id| fetch_my_guild(http, guild_id));
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
    let local_guild_id_set: HashSet<i64> = local_guild_ids
        .iter()
        .cloned()
        .collect();
    let latest_guild_id_set: HashSet<i64> = latest_my_guilds
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
    // guild_memberテーブルからギルドIDごとの「ギルドID, メンバーIDのリスト」のマップを取得
    let rows = select_members(&pool).await?;
    let mut member_ids_map_by_guild: HashMap<i64, Vec<i64>> = HashMap::new();
    for GuildMember { guild_id, member_id, birth: _, last_notified: _ } in rows {
        member_ids_map_by_guild.entry(guild_id).or_default().push(member_id);
    }

    // guild_memberテーブルから取得したギルドIDごとのメンバーIDのリストに
    // 「APIで取得したメンバーIDが存在するか」一つずつ検索
    for MyGuild { id, name: _, members: _ } in &latest_my_guilds {
        let latest_guild_id = id.clone();
        let latest_guild_members_set_by_guild: HashSet<&MyGuildMember> =
            latest_my_guilds
                .iter()
                .filter(|my_guild| my_guild.id == latest_guild_id)
                .flat_map(|my_guild| my_guild.members.iter())
                .collect();

        let local_member_id_set: HashSet<i64> =
            member_ids_map_by_guild
                .get(&latest_guild_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect();

        let latest_member_id_set: HashSet<i64> =
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
            if let Some(_) = member {
                // メンバー情報を更新する内容があれば、ここに処理を置く
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
                    member.birth,
                ).await?;
            }
        }
    }
    // -----------------------------------------------------------------------------------------------------

    Ok(())
}

#[derive(Debug)]
struct MyGuild {
    id: i64,
    name: String,
    members: Vec<MyGuildMember>,
}

#[derive(Debug, Hash, Eq, PartialEq)]
struct MyGuildMember {
    guild_id: i64,
    member_id: i64,
    birth: Option<NaiveDate>,
}

/// ギルドの情報を取得する関数
async fn fetch_my_guild(
    http: &Http,
    guild_id: &GuildId,
) -> anyhow::Result<MyGuild> {
    let partial_guild = http.get_guild(guild_id.clone()).await?;
    let members = partial_guild.members(
        http,
        None,
        None,
    ).await?
        .into_iter()
        .filter_map(|member| {
            Some(MyGuildMember {
                guild_id: i64::from(member.guild_id),
                member_id: i64::from(member.user.id),
                birth: None,
            })
        })
        .collect::<Vec<MyGuildMember>>();

    Ok(MyGuild {
        id: i64::from(partial_guild.id),
        name: partial_guild.name,
        members,
    })
}


/// ボットが所属するギルドIDのリストを取得する関数
async fn fetch_my_guild_ids(http: &Http) -> anyhow::Result<Vec<GuildId>> {
    let guilds = http.get_guilds(None, None).await?;
    Ok(guilds.into_iter().map(|g| g.id).collect())
}


#[derive(Debug, sqlx::FromRow)]
struct GuildMember {
    guild_id: i64,
    member_id: i64,
    birth: Option<NaiveDate>,
    last_notified: Option<NaiveDate>,
}


/// DBからギルドIDのリストを取得するメソッド
async fn select_guild_ids(pool: &PgPool) -> anyhow::Result<Vec<i64>> {
    let guild_ids = sqlx::query_scalar!(
        r#"
        SELECT guild_id::BIGINT FROM guild
        "#
    )
        .fetch_all(pool)
        .await?
        .into_iter()
        .filter_map(|guild_id: i64| Some(guild_id))
        .collect::<Vec<i64>>();
    Ok(guild_ids)
}

/// DBのギルド情報を更新するメソッド
async fn update_guild_by_id(
    pool: &PgPool,
    guild_id: i64,
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
    guild_id: i64,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        DELETE FROM guild_member
        WHERE guild_id = $1
        "#,
        guild_id,
    )
        .execute(pool)
        .await?;

    sqlx::query!(
        r#"
        DELETE FROM guild
        WHERE guild_id = $1
        "#,
        guild_id,
    )
        .execute(pool)
        .await?;
    Ok(())
}

async fn insert_guild_by_id(
    pool: &PgPool,
    guild_id: i64,
    guild_name: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO guild (guild_id, name)
        VALUES ($1, $2)
        ON CONFLICT (guild_id) DO NOTHING
        "#,
        guild_id,
        guild_name,
    )
        .execute(pool)
        .await?;
    Ok(())
}


async fn select_members(
    pool: &PgPool
) -> anyhow::Result<Vec<GuildMember>> {
    let rows = sqlx::query_as::<_, GuildMember>("SELECT * FROM guild_member")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

async fn select_members_by_guild_id(
    pool: &PgPool,
    guild_id: i64,
) -> anyhow::Result<Vec<GuildMember>> {
    let rows = sqlx::query_as::<_, GuildMember>("SELECT * FROM guild_member WHERE guild_id = $1")
        .bind(guild_id)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

async fn select_member_by_id(
    pool: &PgPool,
    guild_id: i64,
    member_id: i64,
) -> anyhow::Result<Option<GuildMember>> {
    let row = sqlx::query_as::<_, GuildMember>(
        "SELECT * FROM guild_member WHERE guild_id = $1 AND member_id = $2",
    )
        .bind(guild_id)
        .bind(member_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}


async fn update_guild_member_birth(
    pool: &PgPool,
    guild_id: i64,
    member_id: i64,
    birth: NaiveDate,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        UPDATE guild_member
        SET birth = $1
        WHERE guild_id = $2 AND member_id = $3
        "#,
        birth,
        guild_id,
        member_id,
    )
        .execute(pool)
        .await?;
    Ok(())
}


async fn delete_guild_member(
    pool: &PgPool,
    guild_id: i64,
    member_id: i64,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        DELETE FROM guild_member
        WHERE guild_id = $1 AND member_id = $2
        "#,
        guild_id,
        member_id,
    )
        .execute(pool)
        .await?;
    Ok(())
}

async fn insert_guild_member(
    pool: &PgPool,
    guild_id: i64,
    member_id: i64,
    birth: Option<NaiveDate>,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO guild_member (guild_id, member_id, birth)
        VALUES ($1, $2, $3)
        ON CONFLICT (guild_id, member_id) DO NOTHING
        "#,
        guild_id,
        member_id,
        birth,
    )
        .execute(pool)
        .await?;
    Ok(())
}

async fn update_guild_member_last_notified(
    pool: &PgPool,
    guild_id: i64,
    member_id: i64,
    last_notified: NaiveDate,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        UPDATE guild_member
        SET last_notified = $1
        WHERE guild_id = $2 AND member_id = $3
        "#,
        last_notified,
        guild_id,
        member_id,
    )
        .execute(pool)
        .await?;
    Ok(())
}

async fn birthday_cron_worker(
    pool: PgPool,
    http: Arc<Http>,
) -> anyhow::Result<()> {
    // 初回の誕生日チェックまでの時間を調節
    let now = Tokyo.from_utc_datetime(&Local::now().naive_utc());
    let noon = NaiveTime::from_hms_opt(21, 50, 0).expect("Invalid time.");
    let today_noon = now.date_naive().and_time(noon);
    let next_noon = if now.naive_local() < today_noon {
        today_noon
    } else {
        // 通知チェックの時刻を過ぎていた場合は、チェック時刻を明日に振替
        today_noon + chrono::Duration::days(1)
    };
    let wait = u64::try_from((next_noon - now.naive_local()).num_seconds().max(0))?;
    sleep(Duration::from_secs(wait)).await;

    // 誕生日チェック
    check_member_birth(&pool, &http).await?;

    // 以降は24時間ごとに正午(12:00)のタイミングで誕生日チェック実行
    loop {
        sleep(Duration::from_secs(60)).await;
        // sleep(Duration::from_secs(60 * 60 * 24)).await;
        check_member_birth(&pool, &http).await?;
    }
}


async fn check_member_birth(pool: &PgPool, http: &Http) -> anyhow::Result<()> {
    let member_ids_map_by_guild = select_members(pool).await?;
    for GuildMember { guild_id, member_id, birth, last_notified } in member_ids_map_by_guild {
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
                update_guild_member_last_notified(
                    pool,
                    i64::from(guild_id),
                    member_id,
                    now.date_naive(),
                ).await?;
            }
        }
    }
    Ok(())
}