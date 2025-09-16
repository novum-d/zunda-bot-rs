mod commands;
mod data;
mod models;
mod usecase;
mod worker;
mod res;

use crate::commands::birth::birth;
use crate::models::common::Data;
use crate::usecase::birth_list_usecase::BirthListUsecase;
use crate::usecase::birth_notify_usecase::BirthNotifyUsecase;
use crate::usecase::birth_reset_usecase::BirthResetUsecase;
use crate::usecase::birth_signup_usecase::BirthSignupUsecase;
use crate::usecase::guild_update_usecase::GuildUpdateUsecase;
use crate::worker::annual_birthday_notifier::AnnualBirthdayNotifier;
use anyhow::Context as _;
use commands::hello::hello;
use poise::serenity_prelude as serenity;
use serenity::model::gateway::GatewayIntents;
use serenity::Client;
use shuttle_runtime::SecretStore;
use sqlx::PgPool;
use std::sync::Arc;

#[shuttle_runtime::main]
async fn main(
    #[shuttle_runtime::Secrets] secrets: SecretStore,
    #[shuttle_shared_db::Postgres()] pool: PgPool, // local_uriを指定するとエラーになるので記述しない
) -> shuttle_serenity::ShuttleSerenity {
    if let Err(e) = sqlx::migrate!("db/migrations").run(&pool).await {
        tracing::error!("Failed to run migrations: {:?}", e);
    }

    let token = secrets
        .get("DISCORD_TOKEN")
        .context("'DISCORD_TOKEN' was not found")?;

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
        .setup(|ctx, _ready, framework| {
            let pool = Arc::new(pool);
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

    let client = Client::builder(&token, intents)
        .framework(framework)
        .await
        .map_err(shuttle_runtime::CustomError::new)?;

    Ok(client.into())
}
