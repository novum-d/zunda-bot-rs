
[![ci-badge][]][ci] [![docs-badge][]][docs] [![rust-version-badge]][rust-version-link]

# zunda-bot-rs

<img src="https://github.com/user-attachments/assets/47b6028f-fea4-4aae-b615-aff4f8e1c197" width="300" />

Rust製のずんだもんDiscord Bot.

## セットアップ
Rustのホスティングサービス`Shuttle`にDiscord Botサービスをデプロイする

> [!NOTE]  
> セットアップ前に以下でリポジトリをダウンロードしておいてください
>```shell
>git clone git@github.com:novum-d/zunda-bot-rs.git
>cd zunda-bot-rs
>```

### 1. Discord Botの用意
1.1. [Discord Developer Portal](https://discord.com/developers/applications)を開き、開発用Discord Botの[トークンを作成](https://note.com/exteoi/n/nf1c37cb26c41)

> [!WARNING]  
>Discordのトークンは、Discord Botやユーザーアカウントを認証・識別するための「秘密鍵」のような文字列です  
>
>このトークンを使ってBotプログラムがDiscord APIにアクセスし、メッセージ送信やイベント受信などの操作が可能になりますが、**第三者に知られるとBotの乗っ取りなどの危険があるため、絶対に公開しないよう注意してください**

1.2. ボットに特権を付与  
   ボット設定画面を開き、メニュー`Bot`で[Privileged Intents](https://discord.com/developers/docs/events/gateway#privileged-intents)に含まれるボットの
   `MESSAGE_CONTENT(メッセージ受取権限)`を有効化


1.3. 不要なセキュリティ設定を解除  
Botをサーバーに追加する際に毎回認証を求められるので、`Requires OAuth2 Code Grant(Botをサーバーに追加する際に認証フローが必須となる設定)`を無効化

1.4. ボットをサーバーに追加  
   メニュー`OAuth`で`OAuth2 URL Generator`で以下を選択したURLをブラウザで開く

   * SCOPES: `bot`
   * BOT PERMISSIONS: `Administrator`  
    
   ```txt
   # URLの例
   https://discord.com/oauth2/authorize?client_id={{CLIENT_ID}}&&permissions={{PERMISSIONS_INTEGER}}&&scope=bot 
   ```
   
   すると、ボットを追加するサーバーを選択する画面が開くので、好きなサーバーを選び、ボットを追加
   
### 2. Shuttle
2.1. Rustのホスティングサービス[Shuttle](https://console.shuttle.dev/)にログイン
```shell
shuttle login
```
[Shuttle CLI](https://docs.shuttle.dev/guides/cli)がない場合は、以下でインストール
```shell
cd zunda-bot-rs
cargo install cargo-shuttle
```

2.2 デプロイ先のプロジェクト(`<project-name>`)を作成
```shell
shuttle project create --name <project-name> 
```
 
### 3. プロジェクトの設定
3.1 既存のプロジェクトと`2.2`で作成したデプロイ先のプロジェクトをリンク
   ```shell
   # 2.2のCLI経由でプロジェクトを作成した場合は不要
   shuttle project link --name <project-name>
   ```
3.2. `1.1.`で作成したトークンを既存のプロジェクトにセット  
   プロジェクトの直下に`Secrets.toml`を作成
   ```shell
   cp Secrets.toml.sample Secrets.toml
   ```
   コピーして作成した`Secrets.toml`にトークンを設定
   ```toml
   DISCORD_TOKEN="{{Your token}}"
   ```
   
### 4. デプロイ
4.1. 作成したBotサービスをデプロイ
   ```shell
   shuttle deploy
   ```

## デバッグ

ローカルで動作をテストしたい場合は、以下を実行
   ```shell
   shuttle run
   ```

## 実行環境

* Rust 1.87.0

## 開発環境
* RustRover 2024.2

## CI/CD

* [GitHub Actions](https://github.com/actions-rs/cargo)
 
>CI/CDは`共有の開発アカウント(Discord, Shuttle)`を使用した環境でのみ実行されるので、利用する際は @novum-d までご連絡ください


[ci]: https://github.com/novum-d/zunda-bot-rs/actions
[ci-badge]: https://img.shields.io/github/actions/workflow/status/novum-d/zunda-bot-rs/discord-bot-deploy.yml?branch=current&style=flat-square

[docs]: https://github.com/novum-d/zunda-bot-rs/tree/main/docs
[docs-badge]: https://img.shields.io/badge/docs-blue


[rust-version-badge]: https://img.shields.io/badge/rust-1.87.0+-93450a.svg?style=flat-square
[rust-version-link]: https://blog.rust-lang.org/2025/05/15/Rust-1.87.0
