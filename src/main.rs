mod data;
mod handler;
mod models;
mod reminder;
mod res;
mod services;
mod usecase;

use crate::models::common::Data;
use crate::reminder::service::ReminderService;
use crate::services::healthcheck::run_healthcheck_server;
use crate::usecase::birth_list_usecase::BirthListUsecase;
use crate::usecase::birth_notify_usecase::BirthNotifyUsecase;
use crate::usecase::birth_reset_usecase::BirthResetUsecase;
use crate::usecase::birth_signup_usecase::BirthSignupUsecase;
use crate::usecase::guild_update_usecase::GuildUpdateUsecase;
use anyhow::Context as _;
use dotenvy::dotenv;
use serenity::all::{ChannelType, Command, CommandOptionType, CreateCommand, CreateCommandOption};
use serenity::http::Http;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env());

    if cfg!(debug_assertions) {
        // 開発時（デバッグビルド）は色付き・整形あり
        subscriber.with_ansi(true).pretty().init();
    } else {
        // 本番（リリースビルド）はシンプルに1行で出力
        subscriber.with_ansi(false).compact().init();
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
    let http = Arc::new(Http::new(&token));

    let pool = Arc::new(pool);
    let birth_list_usecase = BirthListUsecase::new(pool.clone(), http.clone())?;
    let birth_signup_usecase = BirthSignupUsecase::new(pool.clone(), http.clone())?;
    let birth_reset_usecase = BirthResetUsecase::new(pool.clone(), http.clone())?;
    let birth_notify_usecase = BirthNotifyUsecase::new(pool.clone(), http.clone())?;
    let guild_update_usecase = GuildUpdateUsecase::new(pool.clone(), http.clone())?;
    let reminder_service = ReminderService::new(pool.clone(), http.clone())?;

    guild_update_usecase
        .invoke()
        .await
        .context("Failed to sync guilds on startup")?;
    register_global_commands(&http).await?;

    let data = Data {
        birth_list_usecase,
        birth_signup_usecase,
        birth_reset_usecase,
        birth_notify_usecase,
        guild_update_usecase,
        reminder_service,
        discord_http: http,
    };

    run_healthcheck_server(data)
        .await
        .context("Healthcheck server stopped")
}

async fn register_global_commands(http: &Http) -> anyhow::Result<()> {
    let commands = vec![hello_command(), birth_command(), setup_command()];
    Command::set_global_commands(http, commands)
        .await
        .context("Failed to register global slash commands")?;
    Ok(())
}

fn hello_command() -> CreateCommand {
    CreateCommand::new("hello").description("あいさつを返すのだ")
}

fn birth_command() -> CreateCommand {
    CreateCommand::new("birth")
        .description("誕生日通知を操作するのだ")
        .add_option(CreateCommandOption::new(
            CommandOptionType::SubCommand,
            "list",
            "誕生日リストを表示するのだ",
        ))
        .add_option(CreateCommandOption::new(
            CommandOptionType::SubCommand,
            "signup",
            "自身の誕生日を登録するのだ",
        ))
        .add_option(CreateCommandOption::new(
            CommandOptionType::SubCommand,
            "reset",
            "自身の誕生日登録を解除するのだ",
        ))
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::SubCommandGroup,
                "remind",
                "誕生日未登録リマインドを操作するのだ",
            )
            .add_sub_option(CreateCommandOption::new(
                CommandOptionType::SubCommand,
                "resume",
                "誕生日未登録リマインドを再開するのだ",
            )),
        )
}

fn setup_command() -> CreateCommand {
    let channel_option = || {
        CreateCommandOption::new(CommandOptionType::Channel, "channel", "対象チャンネル")
            .required(true)
            .channel_types(vec![ChannelType::Text])
    };

    CreateCommand::new("setup")
        .description("管理者向け設定を操作するのだ")
        .add_option(CreateCommandOption::new(
            CommandOptionType::SubCommand,
            "reminder-channel",
            "誕生日未登録リマインドの送信対象を選ぶのだ",
        ))
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::SubCommand,
                "add-notification-channel",
                "誕生日通知チャンネルを追加するのだ",
            )
            .add_sub_option(channel_option()),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::SubCommand,
                "remove-notification-channel",
                "誕生日通知チャンネルを削除するのだ",
            )
            .add_sub_option(channel_option()),
        )
}
