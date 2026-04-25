mod commands;
mod data;
mod models;
mod res;
mod services;
mod usecase;
mod worker;

use crate::commands::birth::birth;
use crate::models::common::Data;
use crate::services::healthcheck::run_healthcheck_server;
use crate::usecase::birth_list_usecase::BirthListUsecase;
use crate::usecase::birth_notify_usecase::BirthNotifyUsecase;
use crate::usecase::birth_reset_usecase::BirthResetUsecase;
use crate::usecase::birth_signup_usecase::BirthSignupUsecase;
use crate::usecase::guild_update_usecase::GuildUpdateUsecase;
use crate::worker::annual_birthday_notifier::AnnualBirthdayNotifier;
use anyhow::Context as _;
use commands::hello::hello;
use dotenvy::dotenv;
use poise::serenity_prelude as serenity;
use serenity::model::gateway::GatewayIntents;
use serenity::Client;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").context("'DATABASE_URL' was not found")?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("Failed to connect to PostgreSQL")?;

    if let Err(e) = sqlx::migrate!("db/migrations").run(&pool).await {
        tracing::error!("Failed to run migrations: {:?}", e);
    }

    let token = env::var("DISCORD_TOKEN").context("'DISCORD_TOKEN' was not found")?;

    let intents = GatewayIntents::GUILD_MEMBERS // ギルドメンバー情報取得権限
        | GatewayIntents::GUILD_MESSAGES // ギルド内のメッセージイベント受信権限
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                // コマンドはここに追加
                hello(),
                birth(),
            ],
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            let pool = Arc::new(pool.clone());
            Box::pin(async move {
                let birth_list_usecase = BirthListUsecase::new(pool.clone(), ctx.http.clone())?;
                let birth_signup_usecase = BirthSignupUsecase::new(pool.clone(), ctx.http.clone())?;
                let birth_reset_usecase = BirthResetUsecase::new(pool.clone(), ctx.http.clone())?;
                let birth_notify_usecase = BirthNotifyUsecase::new(pool.clone(), ctx.http.clone())?;
                let guild_update_usecase = GuildUpdateUsecase::new(pool.clone(), ctx.http.clone())?;
                guild_update_usecase.invoke().await?;

                tokio::spawn(AnnualBirthdayNotifier::new(birth_notify_usecase));

                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                let data = Data {
                    birth_list_usecase,
                    birth_signup_usecase,
                    birth_reset_usecase,
                    guild_update_usecase,
                };
                Ok(data)
            })
        })
        .build();

    let bot = async {
        let mut client = Client::builder(&token, intents).framework(framework).await?;
        client.start().await?;
        Ok::<(), anyhow::Error>(())
    };

    tokio::select! {
        result = run_healthcheck_server() => result.context("Healthcheck server stopped"),
        result = bot => result.context("Discord bot stopped"),
    }
}
