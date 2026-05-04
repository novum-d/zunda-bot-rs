# ARCHITECTURE.md

## Purpose

This document explains the repository structure, ownership boundaries, and where new code should be placed.

Codex should use this file to avoid unnecessary refactors, large-scale changes, or introducing new directories without
approval.

<!--
このファイルはリポジトリ構成と責務を説明する。
Codex は不要なリファクタ、大規模変更、新規ディレクトリ追加を避けること。
-->

---

## Repository Structure

```text id="2w2xkp"
.
├── src/
│   ├── commands/     # Discord command handlers
│   ├── usecase/      # Application use cases
│   ├── data/         # Repository / DB access
│   ├── services/     # Process-level services (e.g. healthcheck)
│   ├── worker/       # Background job entrypoints
│   ├── models/       # Data structures and domain types
│   └── main.rs       # Application entry point
├── tests/            # Automated tests
├── docs/             # Documentation
├── .github/          # GitHub configuration
└── README.md
```

<!--
commands は Discord コマンド処理。
usecase はアプリケーションのユースケース。
data は DB と外部APIアクセスの橋渡し。
services はプロセス横断の小さなサービス。
worker は定期実行などのバックグラウンド処理。
models は型定義や構造体。
-->

---

## Directory Responsibilities

### src/commands/

Contains Discord command handlers.

Examples:

* ping command
* help command
* slash command handlers
* command-specific validation

Rules:

* Keep handlers small
* Put shared logic into services
* Avoid duplicating validation logic
* Avoid putting large business logic directly in commands

<!--
commands 配下は Discord コマンドの入口を書く。
重い処理や複数コマンドで使う処理は services に寄せる。
-->

---

### src/services/

Contains process-level services.

Examples:

* healthcheck server
* small runtime utilities

Rules:

* Prefer pure functions where possible
* Keep responsibilities small and explicit
* Avoid embedding command business logic here

<!--
services はヘルスチェックなどの小さいプロセス処理を書く。
コマンド固有ロジックは usecase 側に寄せる。
-->

---

### src/usecase/

Contains user-facing and scheduled use cases.

Examples:

* `/birth` command branches (`list`, `signup`, `reset`)
* guild synchronization
* birthday notification execution

Rules:

* Keep per-usecase flow readable
* Move raw SQL and DB access to `src/data/`
* Handle Discord interaction timing (defer/acknowledge) explicitly

<!--
usecase はユーザー操作や定期処理の流れを書く。
DBアクセスは data 側へ分離する。
-->

---

### src/data/

Contains repository and database access implementations.

Examples:

* guild repository
* SQLx query wrappers

Rules:

* Keep queries and persistence concerns in this layer
* Avoid command-specific UI logic
* Return explicit, typed data to usecases

<!--
data は永続化や外部取得の責務を持つ。
UIやコマンド文言は持たない。
-->

---

### src/worker/

Contains long-running background tasks.

Examples:

* annual birthday notifier loop

Rules:

* Keep scheduling logic centralized
* Delegate business decisions to usecases

<!--
worker は定期実行の制御を担当する。
業務ロジックは usecase 側に委譲する。
-->

---

### src/models/

Contains domain models, enums, and shared types.

Examples:

* request/response structs
* config types
* database row types
* enums for command options

Rules:

* Keep models simple
* Avoid putting logic into models unless clearly necessary
* Prefer explicit field names

<!--
models は構造体、enum、型定義を置く場所。
ロジックは最小限にする。
-->

---

## Allowed Architectural Changes

Codex may:

* Add small helper functions
* Add new command handlers
* Update existing usecases for feature work
* Update `src/data/` when persistence or external access must change for the feature
* Update shared models when the behavior requires it
* Add tests
* Improve logging
* Improve error handling
* Extract small shared logic into services

<!--
小規模な共通化、ログ改善、エラーハンドリング改善は許可。
-->

For user-visible feature work, prefer editing the layer that actually owns the behavior instead of limiting changes to
prompts, docs, or command wrappers.

Examples:

* Command option or response flow change: `src/commands/` and/or `src/usecase/`
* Persistence or query change: `src/data/`
* Shared type change required by the feature: `src/models/`

---

## Forbidden Architectural Changes

Codex must not:

* Introduce new top-level directories
* Split the project into multiple crates
* Introduce a database without approval
* Introduce Docker without approval
* Introduce background workers without approval
* Rewrite core application flow
* Rename many files at once
* Move large amounts of code between directories
* Add new infrastructure or deployment logic

<!--
大規模構成変更は禁止。
crate 分割、DB 導入、Docker 導入、デプロイ自動化は承認必須。
-->

---

## Dependency Rules

* Reuse existing crates whenever possible
* Avoid adding new dependencies
* Any new dependency requires human approval
* Prefer standard library solutions first

<!--
依存追加は最終手段。
まず標準ライブラリや既存 crate を利用する。
-->

---

## Testing Expectations

Any change should include appropriate tests when possible.

Preferred order:

1. Unit tests
2. Small integration tests
3. Manual verification steps in PR description

Required commands:

```bash id="bzdas4"
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

<!--
小規模変更は unit test を優先。
必要なら PR に手動確認手順も記載する。
-->

---

## PR Expectations

All PRs should:

* Be Draft PRs
* Be written in Japanese
* Reference the related issue
* Include changed files
* Include test results
* Stay under 300 lines if possible
* Stay within the allowed paths
* Avoid touching unrelated code

<!--
PR は必ず日本語で作成する。
Issue 番号、変更内容、テスト結果を含める。
無関係な変更を混ぜない。
-->

---

## Safety Rules

Codex must immediately stop if:

* A forbidden path would be modified
* A new dependency is required
* More than 10 files would change
* More than 300 lines would be added
* The issue scope becomes unclear
* Human approval is required

<!--
差分が大きすぎる場合、不明確な要件がある場合、承認が必要な場合は停止する。
-->
