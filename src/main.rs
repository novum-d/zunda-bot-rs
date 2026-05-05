mod commands;
mod data;
mod handler;
mod models;
mod reminder;
mod res;
mod services;
mod usecase;
mod worker;

use crate::commands::birth::birth;
use crate::models::common::Data;
use crate::reminder::service::ReminderService;
use crate::services::healthcheck::{run_healthcheck_server, run_passive_healthcheck_server};
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

    tracing_subscriber::fmt()
        .with_ansi(true)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .pretty()
        .init();

    if env::var("ENABLE_DISCORD_BOT").context("'ENABLE_DISCORD_BOT' was not found")? == "false" {
        return run_passive_healthcheck_server()
            .await
            .context("Passive healthcheck server stopped");
    }

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
            event_handler: |ctx, event, _framework, data| {
                Box::pin(async move {
                    match event {
                        serenity::FullEvent::Message { new_message } => {
                            if let Err(e) =
                                handler::message::handle_message(ctx, data, new_message).await
                            {
                                tracing::warn!("birthday reminder message handler failed: {}", e);
                            }
                        }
                        serenity::FullEvent::InteractionCreate {
                            interaction: serenity::Interaction::Component(component),
                        } => match handler::interaction::handle_component_interaction(
                            data, component,
                        )
                        .await
                        {
                            Ok(true) => {}
                            Ok(false) => {}
                            Err(e) => tracing::warn!(
                                "birthday reminder interaction handler failed: {}",
                                e
                            ),
                        },
                        _ => {}
                    }
                    Ok(())
                })
            },
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
                let reminder_service = ReminderService::new(pool.clone(), ctx.http.clone())?;
                let reminder_scan_service = ReminderService::new(pool.clone(), ctx.http.clone())?;
                guild_update_usecase.invoke().await?;

                tokio::spawn(AnnualBirthdayNotifier::run(birth_notify_usecase));
                tokio::spawn(async move {
                    if let Err(e) = run_healthcheck_server(reminder_scan_service).await {
                        tracing::error!("Healthcheck server stopped: {}", e);
                    }
                });

                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                let data = Data {
                    birth_list_usecase,
                    birth_signup_usecase,
                    birth_reset_usecase,
                    guild_update_usecase,
                    reminder_service,
                    discord_http: ctx.http.clone(),
                };
                Ok(data)
            })
        })
        .build();

    let bot = async {
        let mut client = Client::builder(&token, intents)
            .framework(framework)
            .await?;
        client.start().await?;
        Ok::<(), anyhow::Error>(())
    };

    bot.await.context("Discord bot stopped")
}
