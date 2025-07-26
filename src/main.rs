mod commands;

use commands::hello::hello;
use anyhow::Context as _;
use serenity::prelude::*;
use shuttle_runtime::SecretStore;
use poise::serenity_prelude as serenity;
use serenity::model::gateway::GatewayIntents;
use crate::commands::birth::birth;

/// `Data`構造体は、Botコマンド実行時に毎回アクセスできる「ユーザーデータ」を格納するための型
/// この型にフィールドを追加することで、コマンド間で共有したい情報（設定値や状態など）を保持できる
/// `poise`フレームワークでは、各コマンドの`Context`からこの`Data`にアクセスできる
struct Data {} // User data, which is stored and accessible in all command invocations
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;


#[shuttle_runtime::main]
async fn main(
    #[shuttle_runtime::Secrets] secrets: SecretStore,
) -> shuttle_serenity::ShuttleSerenity {
    // `Secrets.toml`からトークンを取得
    let token = secrets
        .get("DISCORD_TOKEN")
        .context("'DISCORD_TOKEN' was not found")?;

    // インテントを設定
    let intents = GatewayIntents::GUILD_MESSAGES
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
