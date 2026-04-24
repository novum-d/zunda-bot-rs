
[![ci-badge][]][ci] [![docs-badge][]][docs] [![rust-version-badge]][rust-version-link]

# zunda-bot-rs

<img src="https://github.com/user-attachments/assets/47b6028f-fea4-4aae-b615-aff4f8e1c197" width="300" />

Rust製のずんだもんDiscord Bot.

## セットアップ
Discord Botサービスをローカル、Docker、または Google Cloud Run で実行する

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
   
### 2. 環境変数の設定
2.1. `1.1.`で作成したトークンと PostgreSQL の接続先を環境変数に設定
   ```shell
   export DISCORD_TOKEN="{{Your token}}"
   export DATABASE_URL="postgres://{{user}}:{{password}}@{{host}}:5432/{{database}}"
   ```

2.2. SQLx のオフラインメタデータを使ってビルドする場合は、以下も設定
   ```shell
   export SQLX_OFFLINE=true
   ```

### 3. ローカル実行
```shell
cargo run
```

起動時に `db/migrations` のマイグレーションを実行します。

### 4. Docker 実行
```shell
docker build -t zunda-bot-rs .
docker run --env DISCORD_TOKEN --env DATABASE_URL -p 8080:8080 zunda-bot-rs
```

### 5. Cloud Run へのデプロイ
Artifact Registry にイメージを push して Cloud Run にデプロイします。

```shell
gcloud builds submit --tag {{REGION}}-docker.pkg.dev/{{PROJECT_ID}}/{{REPOSITORY}}/zunda-bot-rs
gcloud run deploy zunda-bot-rs \
  --image {{REGION}}-docker.pkg.dev/{{PROJECT_ID}}/{{REPOSITORY}}/zunda-bot-rs \
  --region {{REGION}} \
  --set-secrets DISCORD_TOKEN={{DISCORD_TOKEN_SECRET}}:latest,DATABASE_URL={{DATABASE_URL_SECRET}}:latest
```

Cloud Run の起動確認用に、コンテナは `PORT` 環境変数のポートで HTTP 200 を返します。

## デバッグ

ローカルで動作をテストしたい場合は、以下を実行
   ```shell
   SQLX_OFFLINE=true cargo run
   ```

## 実行環境

* Rust 1.87.0

## 開発環境
* RustRover 2024.2

## CI/CD

* [GitHub Actions](https://github.com/actions-rs/cargo)
 
>CI/CDは共有の開発アカウントを使用した環境でのみ実行されるので、利用する際は @novum-d までご連絡ください


[ci]: https://github.com/novum-d/zunda-bot-rs/actions
[ci-badge]: https://img.shields.io/github/actions/workflow/status/novum-d/zunda-bot-rs/discord-bot-deploy.yml?branch=main&style=flat-square

[docs]: https://github.com/novum-d/zunda-bot-rs/tree/main/docs
[docs-badge]: https://img.shields.io/badge/docs-blue


[rust-version-badge]: https://img.shields.io/badge/rust-1.87.0+-93450a.svg?style=flat-square
[rust-version-link]: https://blog.rust-lang.org/2025/05/15/Rust-1.87.0
