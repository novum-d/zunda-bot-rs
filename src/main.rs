mod commands;
mod data;
mod models;
mod res;
mod usecase;
mod worker;

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
use sqlx::PgPool;
use std::env;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tokio::spawn(async {
        if let Err(e) = run_healthcheck_server().await {
            tracing::error!("Healthcheck server stopped: {:?}", e);
        }
    });

    let database_url = env::var("DATABASE_URL").context("'DATABASE_URL' was not found")?;
    let pool = PgPool::connect(&database_url)
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

    let mut client = Client::builder(&token, intents)
        .framework(framework)
        .await?;

    client.start().await?;

    Ok(())
}

async fn run_healthcheck_server() -> anyhow::Result<()> {
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;

    loop {
        let (mut stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            let _ = stream.write_all(healthcheck_response()).await;
        });
    }
}

fn healthcheck_response() -> &'static [u8] {
    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK"
}

#[cfg(test)]
mod tests {
    use super::healthcheck_response;

    #[test]
    fn healthcheck_response_returns_ok() {
        let response = healthcheck_response();

        assert!(response.starts_with(b"HTTP/1.1 200 OK"));
        assert!(response.ends_with(b"\r\n\r\nOK"));
    }
}
