You are implementing a GitHub issue for this repository.

Before making changes, read:
- AGENTS.md
- docs/ai/SKILLS.md
- docs/ai/ARCHITECTURE.md
- docs/ai/TESTING.md
- docs/ai/DECISIONS.md

Issue title:
Discord Interaction Webhook の入口と署名検証を追加する

Issue body:
## Context

Discord Gateway ではなく Discord Interactions Endpoint URL で slash command / interaction を受けるための HTTP 入口を追加する。

現在は `src/main.rs` で serenity/poise の Gateway client を起動し、`src/services/healthcheck.rs` の HTTP サーバは healthcheck と `POST /internal/reminder/scan` のみを扱っている。

Discord 公式仕様上、Interactions Endpoint URL には以下が必須。

* `POST /interactions` で `type: 1` の PING を受け、`{"type":1}` を返す
* `X-Signature-Ed25519` と `X-Signature-Timestamp` を使って Discord 署名を検証する

独自認証は追加しないが、Discord 署名検証は必須要件として実装する。

## Goal

Cloud Run 上で Discord Interaction webhook を受信できる最小の HTTP endpoint を追加し、Discord Developer Portal に Interactions Endpoint URL として登録できる状態にする。

## Task Type

- [x] Implementation
- [ ] Maintenance / Docs

## Non-goals

今回やらないこと

* 既存 slash command の実行移植
* Gateway の削除
* Cloud Run / Cloud Scheduler / GitHub Actions / deploy 設定の変更
* message create / reaction event の webhook 対応
* 独自認証や追加の API key 認証

## Files or directories allowed to change

* src/services/
* src/models/
* tests/
* README.md
* docs/
* Cargo.toml
* Cargo.lock

## Acceptance Criteria

* [ ] `POST /interactions` が HTTP で受けられる
* [ ] Discord PING interaction `{"type":1}` に `200` と `{"type":1}` を返す
* [ ] `X-Signature-Ed25519` / `X-Signature-Timestamp` / raw body による署名検証を行う
* [ ] 署名が不正な場合は `401` を返す
* [ ] 未対応 interaction type は既存処理を壊さず、明示的なエラーまたは未対応応答を返す
* [ ] 既存の `GET /` healthcheck と `POST /internal/reminder/scan` を壊さない
* [ ] 必要な依存追加がある場合は最小限にし、PR に理由を書く
* [ ] cargo fmt --check が通る
* [ ] cargo clippy --all-targets --all-features -- -D warnings が通る
* [ ] cargo test が通る
* [ ] 既存機能を壊さない
* [ ] README が必要なら更新される
* [ ] 実装依頼の場合、`.codex` やドキュメントだけで完了扱いにしない
* [ ] 実装依頼の場合、`src/**` または `tests/**` に関連差分が入る

## Similar existing implementation

* src/services/healthcheck.rs
* src/main.rs



Detected issue kind:
codex

Rules:
- Follow AGENTS.md
- If the issue explicitly allows scope overrides, follow the issue scope
- Do not modify forbidden files
- For implementation issues, do not finish with only .codex, prompt, or documentation changes
- For implementation issues, include at least one behavior-related change under src/** and validate it in tests/** when possible
- If repository policy blocks the required code path, stop and report the blocked path clearly
- Do not touch workflows, deploy, infra, or secrets unless the issue explicitly allows it
- Write PR title and body in Japanese
- Run cargo fmt --check, cargo clippy --all-targets --all-features -- -D warnings, cargo test when possible
- If fmt, clippy, or test fails, record the failure and continue
- Stop if approval is required
- Keep changes focused on the issue only
