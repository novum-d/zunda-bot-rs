# AGENTS.md

## Purpose

This repository uses Codex to automate:

1. Manual issue creation
2. Implementation
3. Draft PR creation
4. Human review and merge

Codex must prioritize safety, small changes, low token usage, and minimal blast radius.

<!--
このリポジトリでは Codex を使って Issue → 実装 → Draft PR 作成までを自動化する。
安全性・小さな差分・低トークン消費・限定的な変更範囲を最優先とする。
-->

---

## Required AI Documentation

Before making any changes, Codex must read the following files:

* docs/ai/SKILLS.md
* docs/ai/ARCHITECTURE.md
* docs/ai/TESTING.md
* docs/ai/DECISIONS.md
* docs/about_git.md

<!--
変更前に上記ファイルを必ず読むこと。
実装方針、責務、テスト方針、プロジェクト概要、禁止事項を理解した上で作業する。
-->

---

## Allowed Changes

Codex may only modify the following paths unless explicitly allowed in the issue:

* src/commands/**
* src/services/**
* src/usecase/**
* src/data/**
* src/models/**
* src/worker/**
* tests/**
* README.md
* docs/**

<!--
Issue に明示されていない限り、上記以外のパスは変更してはいけない。
-->

For implementation requests, Codex should prefer changing behavior in `src/**` and validating it in `tests/**`.

Prompt/config/docs-only changes are not considered a complete implementation unless the issue explicitly asks for
documentation or Codex configuration updates.

If a requested feature or bug fix cannot be completed without touching files outside the allowed paths, Codex must stop
and explain the blocked path instead of finishing with only `.codex` or documentation changes.

---

## Forbidden Changes

Codex must never modify:

* .github/workflows/**
* infra/**
* deploy/**
* secrets/**
* .env
* .env.*
* Cargo.toml
* Cargo.lock
* Dockerfile
* docker-compose.yml
* release scripts
* production configs

Any attempt to modify forbidden files must fail immediately.

<!--
上記ファイルやディレクトリは変更禁止。
変更しようとした場合は即座に停止すること。
-->

---

## Human Approval Required

The following changes always require explicit human approval:

* Cargo.toml changes
* Cargo.lock changes
* New dependencies
* GitHub Actions workflow changes
* Deployment changes
* Infrastructure changes
* Database schema changes
* Secret-related files
* Release automation changes

<!--
以下は必ず人間の承認が必要。
自動で進めてはいけない。
-->

---

## Allowed Scope

Codex should prefer:

* Small bug fixes
* Logging improvements
* Error message improvements
* Tests
* Documentation updates
* Small refactors
* Small CI improvements

Codex should avoid:

* Large architectural changes
* Multi-directory refactors
* Renaming many files
* Rewriting core logic
* Broad dependency upgrades
* Multi-feature PRs

<!--
小規模修正・ログ改善・テスト追加・README 修正などを優先する。
大規模設計変更や複数機能をまとめた PR は避ける。
-->

---

## Required Checks

Before opening a PR, Codex must run:

```bash id="w3q9pb"
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

If any command fails, record the failure in the PR and continue unless the issue explicitly requires a clean pass.

<!--
fmt / clippy / test のいずれかが失敗した場合でも、結果を記録した上で継続してよい。
Issue 側で成功必須と明記されている場合のみ停止する。
-->

---

## Forbidden Commands

The following commands must never be used:

```text id="r6u1ny"
rm -rf
sudo
curl | sh
wget | sh
chmod 777
mkfs
dd if=
shutdown
reboot
systemctl
docker system prune
git push --force
git reset --hard
nohup
tmux
screen
disown
&
```

<!--
破壊的コマンド、権限昇格、バックグラウンド実行は禁止。
-->

<!--

---

## PR Size Limits

Codex must keep changes small.

Maximum allowed limits:

```yaml id="u2k8cd"
max_changed_files: 10
max_added_lines: 300
max_deleted_lines: 150
```

If a task exceeds these limits, Codex should stop and request the work be split into smaller issues.

変更ファイル数・追加行数・削除行数が多すぎる場合は停止し、Issue を分割すること。

-->

---

## Test Safety Rules

Codex must never:

* Delete files under tests/
* Remove assertions
* Add #[ignore]
* Disable CI checks
* Reduce lint strictness
* Add allow(warnings)
* Remove existing test coverage

<!--
テスト削除・assert 削除・lint 緩和は禁止。
-->

---

## Pull Request Rules

All PRs created by Codex must:

* Be Draft PRs
* Be written in Japanese
* Use a clear PR title that states objective + key change (avoid vague titles like `Issue #xx の対応`)
* Reference the related issue
* Include a short summary
* Include `作業内容` section
* Include `作業意図` section
* Include `手動で次にするべき作業` section
* Include changed files
* Include test results
* Include known limitations
* Remain under 300 lines if possible
* Avoid renaming files unless necessary

<!--
PR のタイトル・本文・概要・制限事項は日本語で記載すること。
-->

Example PR format:

```md id="n8f4et"
## 概要

- Discord API エラー時のログ出力を追加
- コマンド実行時のエラーハンドリングを改善

## 作業内容

- 何を変更したかを箇条書きで記載

## 作業意図

- なぜその変更が必要かを簡潔に記載

## 手動で次にするべき作業

- レビュアー/運用担当が次に行う確認手順を記載

## 変更ファイル

- src/commands/ping.rs
- tests/ping_test.rs

## テスト結果

- cargo fmt --check
- cargo clippy --all-targets --all-features -- -D warnings
- cargo test

## 制限事項

- retry 処理は追加していない
```

---

## Branch Rules

Codex must never:

* Push directly to main
* Merge PRs
* Convert Draft PRs to Ready for Review
* Use force push
* Rebase shared branches

Codex must follow branch naming and git operation rules documented in `docs/about_git.md`.

<!--
main への直接 push・自動 merge・force push は禁止。
-->

---

## Timeout Rules

Codex should stop work if:

* More than 15 minutes have elapsed
* More than 10 files would be changed
* More than 300 lines would be added
* The scope becomes unclear
* Human approval is required

<!--
時間超過・差分超過・不明確な要件・人間承認が必要な場合は停止する。
-->

---

## Preferred Strategy

Codex should:

1. Read the issue carefully
2. Read all required AI documentation
3. Limit work to allowed paths
4. Reuse existing patterns
5. Make the smallest possible change
6. Run required checks
7. Open a Draft PR
8. Let a human review and merge

## Documentation Sync Rule

When changes affect behavior, operations, or architecture, Codex must update documentation as needed in the same task:

* `README.md` (setup/deployment/operations)
* `docs/uml/**` (flow and ER diagrams)

<!--
Issue と docs/ai 配下のファイルを読み、既存実装を参考にしながら、最小限の変更で Draft PR を作成すること。
-->
