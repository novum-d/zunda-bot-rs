You are implementing a GitHub issue for this repository.

Before making changes, read:
- AGENTS.md
- docs/ai/SKILLS.md
- docs/ai/ARCHITECTURE.md
- docs/ai/TESTING.md
- docs/ai/DECISIONS.md

Issue title:
[🏗️ 雑用] 誕生日未登録ユーザー向けリマインド機能の要件整理

Issue body:
## Context

誕生日未登録ユーザーへのリマインド機能を検討しているが、元の要件は以下を同時に含んでいた。

- データモデル変更
- 定期実行の追加
- Bot チャンネル投稿
- 通知停止 / 再開 UI
- Cloud Scheduler / Cloud Run 前提の運用設計

現行の Codex 運用ルールでは、これらを 1 Issue でまとめて自動実装するのはスコープ超過になる。

## Goal

この Issue では、リマインド機能を Codex が安全に実装できる単位へ分割し、実装前提を整理する。

## Task Type

- [ ] Implementation
- [x] Maintenance / Docs

## Non-goals

- 本機能の一括実装
- DB schema 変更の自動実装
- GCP / Cloud Scheduler / Cloud Run 構成の自動実装
- 環境変数追加を伴う本番運用変更

## Files or directories allowed to change

- docs/
- README.md

## Required approvals / blocked items

以下は現行ルール上、別 Issue または人手承認が必要。

- データモデル / schema 変更
- 新しい定期実行の追加
- インフラ / デプロイ / Cloud Scheduler 関連変更
- 本番設定や環境変数の追加

## Proposed split

以下のように分割して扱う。

1. 要件整理と仕様確定
2. 通知対象判定ロジックの実装
3. 通知停止 / 再開の操作設計
4. 定期実行方式の決定と承認
5. 必要なら schema 変更の承認付き Issue 作成

## Acceptance Criteria

- [ ] この Issue が「実装 Issue」ではなく「要件整理 / 分割 Issue」であることが明確
- [ ] 自動実装できない項目が明記されている
- [ ] 後続の小さな実装 Issue に分割できる状態になっている

## Notes

- 実装を Codex に依頼する場合は、後続 Issue を 1 つずつ小さく作成する
- 実装系 Issue では `src/**` または `tests/**` の差分を必須にする


Detected issue kind:
chore

Rules:
- Follow AGENTS.md
- If the issue explicitly allows scope overrides, follow the issue scope
- Keep changes under 500 added lines
- Do not modify forbidden files
- For implementation issues, do not finish with only .codex, prompt, or documentation changes
- For implementation issues, include at least one behavior-related change under src/** and validate it in tests/** when possible
- If repository policy blocks the required code path, stop and report the blocked path clearly
- Do not touch workflows, deploy, infra, or secrets unless the issue explicitly allows it
- Write PR title and body in Japanese
- Run cargo fmt --check, cargo clippy --all-targets --all-features -- -D warnings, cargo test
- Stop if approval is required
- Keep changes focused on the issue only
