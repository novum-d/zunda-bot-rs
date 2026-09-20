# DECISIONS.md

## Purpose

This document records important technical decisions for the repository.

Codex should use this file to avoid introducing changes that conflict with existing project direction.

<!--
このファイルは技術的な判断や方針を記録する。
Codex は既存方針と矛盾する変更を避けること。
-->[DECISIONS.md](DECISIONS.md)

---

## Dependency Policy

We avoid adding dependencies unless there is a strong reason.

Prefer using:

* Existing crates already used in the repository
* Rust standard library
* Small utility functions over introducing new libraries

New dependencies are allowed when they are necessary for the requested implementation, but they must stay minimal and be
explained in the PR. Broad dependency upgrades should be avoided.

<!--
小さな機能のために依存を増やさない。
まず既存 crate と標準ライブラリを使う。
必要な依存追加は最小限にし、PR に理由を記載する。
-->

---

## Database Policy

Current persistence is file-based by default.

Do not introduce a new database unless the issue explicitly authorizes the schema and persistence change.

Do not introduce:

* PostgreSQL
* MySQL
* Redis
* SQLite
* External managed databases

unless explicitly authorized by the issue.

<!--
現在はファイルベース管理を前提とする。
Issue で明示されている場合のみ DB を導入する。
-->

---

## Deployment Policy

Deployment is handled manually by default.

Do not add deployment automation unless the issue explicitly authorizes it.

Do not introduce:

* Auto deploy workflows
* Production release workflows
* Kubernetes
* Terraform
* Infrastructure-as-code tools

unless explicitly authorized by the issue.

<!--
デプロイは手動前提。
Issue で明示されている場合のみ deploy workflow や IaC を追加する。
-->

---

## Docker Policy

Docker is not required by default.

Do not add:

* Dockerfile
* docker-compose.yml
* Container build workflows

unless explicitly authorized by the issue.

<!--
Docker はデフォルトでは不要。
Issue で明示されている場合のみ Docker 関連差分を追加する。
-->

---

## CI Policy

CI should stay simple.

Allowed CI scope:

* cargo fmt
* cargo clippy
* cargo test

Do not add:

* Heavy matrix builds
* Multi-platform builds
* Complex release pipelines
* Automatic deployment

unless explicitly authorized by the issue.

<!--
CI は最小限に保つ。
fmt / clippy / test のみを基本とする。
-->

---

## Refactor Policy

Prefer small incremental refactors.

Avoid:

* Large file renames
* Moving many files at once
* Rewriting core logic
* Changing architecture broadly

<!--
大規模リファクタは禁止寄り。
小さな改善を積み重ねる。
-->

---

## Logging Policy

Logging should be useful but minimal.

Prefer:

* Error logs with enough context
* Warnings for recoverable issues
* Avoiding excessive debug logs

<!--
ログは必要十分にする。
debug ログを増やしすぎない。
-->

---

## Testing Policy

Tests are required for behavior changes when possible.

Prefer:

1. Unit tests
2. Small integration tests
3. Manual verification steps in PR description

Do not disable tests to make CI pass.

<!--
挙動変更には可能な限りテストを追加する。
失敗テストを隠して通すことは禁止。
-->

---

## Pull Request Policy

All PRs should:

* Be written in Japanese
* Be Draft PRs
* Stay under 300 lines if possible
* Avoid unrelated changes
* Include test results
* Include limitations if any

<!--
PR は日本語で、小さく、無関係な変更を含めない。
-->

---

## Scope Policy

Prefer solving one issue per PR.

Avoid:

* Combining multiple features
* Mixing refactors and new features
* Large multi-directory changes

<!--
1 PR = 1 Issue を基本とする。
複数機能をまとめない。
-->

For implementation issues, a PR should normally include behavior changes in repository code, not only prompt,
configuration, or documentation edits.

If Codex is blocked because the issue does not authorize a required restricted path, it should stop and report that
missing authorization clearly. If the issue explicitly authorizes the restricted path, Codex may proceed while keeping
the change focused on that issue.
