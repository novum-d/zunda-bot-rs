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
│   ├── services/     # Shared business logic
│   ├── models/       # Data structures and domain types
│   ├── utils/        # Small helper functions
│   └── main.rs       # Application entry point
├── tests/            # Automated tests
├── docs/             # Documentation
├── .github/          # GitHub configuration
└── README.md
```

<!--
commands は Discord コマンド処理。
services は共通ロジック。
models は型定義や構造体。
utils は小さな補助関数。
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

Contains shared business logic used across multiple commands.

Examples:

* message formatting
* API access
* persistence
* shared validation
* scheduling logic

Rules:

* Prefer pure functions where possible
* Avoid direct Discord-specific logic
* Keep services reusable
* Avoid side effects unless required

<!--
services は共通処理を書く場所。
Discord 固有処理は commands 側に残す。
副作用は必要最小限にする。
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

### src/utils/

Contains small reusable helper functions.

Examples:

* date formatting
* string parsing
* small conversion helpers

Rules:

* Keep utilities small
* Avoid putting business logic into utils
* Do not create generic abstractions unless clearly reused

<!--
utils は小さな補助関数のみ。
何でも utils に入れず、再利用性が高いものだけ置く。
-->

---

## Allowed Architectural Changes

Codex may:

* Add small helper functions
* Add new command handlers
* Add tests
* Improve logging
* Improve error handling
* Extract small shared logic into services

<!--
小規模な共通化、ログ改善、エラーハンドリング改善は許可。
-->

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
