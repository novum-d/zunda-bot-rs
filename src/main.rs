mod data;
mod handler;
mod models;
mod reminder;
mod res;
mod services;
mod usecase;

use crate::models::common::Data;
use crate::reminder::service::ReminderService;
use crate::services::healthcheck::{new_shared_data, run_healthcheck_server_with_shared_data};
use crate::usecase::birth_list_usecase::BirthListUsecase;
use crate::usecase::birth_notify_usecase::BirthNotifyUsecase;
use crate::usecase::birth_reset_usecase::BirthResetUsecase;
use crate::usecase::birth_signup_usecase::BirthSignupUsecase;
use crate::usecase::guild_update_usecase::GuildUpdateUsecase;
use crate::usecase::seven_days_usecase::SevenDaysUsecase;
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

    let shared_data = new_shared_data();
    let init_shared_data = shared_data.clone();
    tokio::spawn(async move {
        match initialize_data().await {
            Ok(data) => {
                match init_shared_data.write() {
                    Ok(mut guard) => {
                        *guard = Some(data.clone());
                    }
                    Err(_) => {
                        tracing::error!("shared data lock poisoned before publishing app data");
                        return;
                    }
                }
                run_startup_tasks(data).await;
            }
            Err(e) => {
                tracing::error!("Application initialization failed: {:?}", e);
            }
        }
    });

    run_healthcheck_server_with_shared_data(shared_data)
        .await
        .context("Healthcheck server stopped")
}

async fn initialize_data() -> anyhow::Result<Data> {
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

    Ok(Data {
        birth_list_usecase,
        birth_signup_usecase,
        birth_reset_usecase,
        birth_notify_usecase,
        guild_update_usecase,
        reminder_service,
        discord_http: http,
        seven_days_usecase: SevenDaysUsecase::from_env(pool)?,
    })
}

async fn run_startup_tasks(data: Data) {
    if let Err(e) = data.guild_update_usecase.invoke().await {
        tracing::error!("Failed to sync guilds on startup: {:?}", e);
    }

    if let Err(e) = ensure_application_id(&data.discord_http).await {
        tracing::error!("Failed to resolve Discord application id: {:?}", e);
        return;
    }

    if let Err(e) = register_global_commands(&data.discord_http).await {
        tracing::error!("Failed to register global slash commands: {:?}", e);
    }
}

async fn ensure_application_id(http: &Http) -> anyhow::Result<()> {
    if http.application_id().is_some() {
        return Ok(());
    }

    let application = http
        .get_current_application_info()
        .await
        .context("Failed to fetch current Discord application info")?;
    http.set_application_id(application.id);
    Ok(())
}

async fn register_global_commands(http: &Http) -> anyhow::Result<()> {
    let commands = vec![
        hello_command(),
        birth_command(),
        setup_command(),
        seven_days_command(),
    ];
    Command::set_global_commands(http, commands)
        .await
        .context("Failed to register global slash commands")?;
    Ok(())
}

fn seven_days_command() -> CreateCommand {
    let subject_command = |name, description, kind, subject_name, subject_description| {
        CreateCommandOption::new(CommandOptionType::SubCommand, name, description).add_sub_option(
            CreateCommandOption::new(kind, subject_name, subject_description).required(true),
        )
    };

    CreateCommand::new("7dtd")
        .description("7 Days to Die 専用サーバーを操作するのだ")
        .add_option(CreateCommandOption::new(
            CommandOptionType::SubCommand,
            "start",
            "サーバーを起動するのだ",
        ))
        .add_option(CreateCommandOption::new(
            CommandOptionType::SubCommand,
            "status",
            "サーバーの状態を確認するのだ",
        ))
        .add_option(CreateCommandOption::new(
            CommandOptionType::SubCommand,
            "stop",
            "サーバーを安全に停止するのだ",
        ))
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::SubCommand,
                "setup",
                "7DTD操作チャンネルを設定するのだ",
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::Channel,
                    "channel",
                    "7DTDコマンドを実行するチャンネル",
                )
                .required(true)
                .channel_types(vec![ChannelType::Text]),
            ),
        )
        .add_option(
            subject_command(
                "allow-user",
                "7DTDを操作できるユーザーを追加・更新するのだ",
                CommandOptionType::User,
                "user",
                "対象ユーザー",
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::Boolean,
                    "admin",
                    "起動・停止と設定変更を許可するか",
                )
                .required(true),
            ),
        )
        .add_option(
            subject_command(
                "allow-role",
                "7DTDを操作できるロールを追加・更新するのだ",
                CommandOptionType::Role,
                "role",
                "対象ロール",
            )
            .add_sub_option(
                CreateCommandOption::new(
                    CommandOptionType::Boolean,
                    "admin",
                    "起動・停止と設定変更を許可するか",
                )
                .required(true),
            ),
        )
        .add_option(subject_command(
            "remove-user",
            "7DTDを操作できるユーザーから削除するのだ",
            CommandOptionType::User,
            "user",
            "対象ユーザー",
        ))
        .add_option(subject_command(
            "remove-role",
            "7DTDを操作できるロールから削除するのだ",
            CommandOptionType::Role,
            "role",
            "対象ロール",
        ))
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
