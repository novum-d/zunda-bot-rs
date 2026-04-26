You are implementing a GitHub issue for this repository.

Before making changes, read:
- AGENTS.md
- docs/ai/SKILLS.md
- docs/ai/ARCHITECTURE.md
- docs/ai/TESTING.md
- docs/ai/DECISIONS.md

Issue title:
Shuttle依存を排除し、Discord botをGCP (Cloud Run) に移行する

Issue body:
## Context

現在のDiscord botは Shuttle 上で動作しています。  
Shuttle 固有のランタイムや Secrets 管理に依存しているため、GCP（Cloud Run）へ移行できるようにしたいです。

---

## Goal

Shuttle 依存を排除し、GCP（Cloud Run）上で動作する構成に移行する。

主な対応内容：
- Docker ベースで実行可能にする
- 環境変数ベースの Secrets 管理に変更する
- 通常の tokio ランタイムで起動できるようにする

---

## Non-goals

- 機能追加
- UI変更
- DBスキーマ変更

---

## Scope Override（このIssueでは例外的に許可）

この Issue に限り、以下のファイル変更を許可する：

- `Cargo.toml`（Shuttle 依存の削除・追加）
- `Cargo.lock`（`Cargo.toml`変更に伴う更新）
- `Dockerfile` の新規作成
- `docker-compose.yml` の新規作成（任意）
- Cloud Run 用の設定や README 追記

---

## Files or directories allowed to change

- `src/**`
- `Cargo.toml`
- `Cargo.lock`
- `Dockerfile`（新規作成可）
- `docker-compose.yml`（任意）
- `README.md`

---

## Acceptance Criteria

### 開発時（PR作成時）

- [ ] Shuttle 依存（`shuttle_runtime`, `shuttle_serenity`, `shuttle_shared_db` など）が削除されている
- [ ] `DISCORD_TOKEN` を環境変数から取得するようになっている（Secrets.toml 不使用）
- [ ] `tokio` ベースで `cargo run` が可能
- [ ] `Dockerfile` が `docker build` 可能
- [ ] 既存のインターフェースが壊れていない
- [ ] README に Cloud Run デプロイ手順が追記されている
- [ ] `cargo fmt --check` / `cargo clippy --all-targets --all-features -- -D warnings` / `cargo test` が通る

---

### マージ前（人間レビュー）

- [ ] Cloud Run 上でコンテナが正常起動する
- [ ] Discord Bot が正常に接続できる
- [ ] 最低1〜2コマンドの動作確認ができる

---

## Notes

- Cloud Run へのデプロイ自体はこのIssueでは必須ではない（別Issueで対応可）
- 本PRでは「ローカル + Docker で動く状態」をゴールとする

---

## Similar existing implementation

- 既存の Shuttle 実装を参考にしつつ置き換える

Rules:
- Follow AGENTS.md
- If the issue explicitly allows scope overrides, follow the issue scope
- Keep changes under 300 lines
- Do not modify forbidden files
- Do not touch workflows, deploy, infra, or secrets
- Write PR title and body in Japanese
- Run cargo fmt --check, cargo clippy --all-targets --all-features -- -D warnings, cargo test
- Stop if approval is required
- Keep changes focused on the issue only
