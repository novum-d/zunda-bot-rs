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

現在、このDiscord botは Shuttle を用いてデプロイされている。
Shuttle 固有のランタイムや Secrets 管理に依存しているため、
他環境（GCP）への移行が困難な状態になっている。

## Goal

Shuttle 依存を排除し、GCP (Cloud Run) 上で動作するようにする。

具体的には：
- Docker 化してデプロイ可能にする
- 環境変数ベースで Secrets を扱う
- 通常の tokio ランタイムで起動する

## Non-goals

- 新機能の追加
- コマンド仕様の変更
- DB構成の変更（必要最低限を除く）

## Files or directories allowed to change

* src/
* Cargo.toml
* Dockerfile（新規追加可）
* README.md

## Acceptance Criteria

* [ ] Shuttle 依存（shuttle_runtime 等）が削除されている
* [ ] `cargo run` でローカル起動できる
* [ ] Docker build が成功する
* [ ] Cloud Run 上で起動できる
* [ ] 環境変数から Discord Token を取得できる
* [ ] 既存コマンドが正常に動作する
* [ ] README にデプロイ手順が追加されている

## Similar existing implementation

なし（Shuttle 依存のため）

Rules:
- Follow AGENTS.md
- Keep changes under 300 lines
- Do not modify forbidden files
- Do not add dependencies
- Do not change Cargo.toml or Cargo.lock
- Do not touch workflows, deploy, infra, or secrets
- Write PR title and body in Japanese
- Run cargo fmt, cargo clippy, cargo test
- Stop if approval is required
- Keep changes focused on the issue only
