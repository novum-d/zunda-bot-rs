# AGENTS.md

## Purpose

This repository uses Codex to automate:

1. Manual issue creation
2. Implementation
3. Draft PR creation
4. Human review and merge

Codex must prioritize safety, focused changes, and minimal blast radius. Small changes are preferred, but Codex may
complete larger implementation issues when the issue explicitly defines the broad scope and allowed paths.

<!--
このリポジトリでは Codex を使って Issue → 実装 → Draft PR 作成までを自動化する。
安全性・焦点の合った差分・限定的な変更範囲を最優先とする。
Issue が広い scope と変更許可パスを明示している場合は、大きめの実装も完了まで進めてよい。
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

Codex may modify the following baseline paths for ordinary implementation issues:

* src/commands/**
* src/services/**
* src/usecase/**
* src/data/**
* src/models/**
* src/worker/**
* tests/**
* README.md
* docs/**
* Cargo.toml
* Cargo.lock

<!--
通常の実装 Issue では上記を基本の変更可能範囲とする。
-->

Paths outside this list may be changed when the issue explicitly authorizes them in a section such as
`Allowed restricted paths`, `Scope Override`, or `Files or directories allowed to change`.

In non-interactive GitHub Actions runs, explicit authorization in the issue body is the human approval source. Codex
should not stop to ask for separate approval when the issue clearly authorizes the restricted path.

For implementation requests, Codex should prefer changing behavior in `src/**` and validating it in `tests/**`.

Prompt/config/docs-only changes are not considered a complete implementation unless the issue explicitly asks for
documentation or Codex configuration updates.

If a requested feature or bug fix cannot be completed without touching a restricted path that the issue did not
authorize, Codex must stop and explain the blocked path instead of finishing with only `.codex` or documentation changes.

---

## Absolute Forbidden Changes

Codex must never modify:

* secrets/**
* .env
* .env.*
* files containing real tokens, passwords, private keys, credentials, or production secret values
* generated private backups, game saves, player data, or logs containing private data

Codex must never commit secret values, print secret values into logs, or add generated private runtime data to the
repository.

<!--
秘密情報や個人データは絶対にコミットしない。
-->

---

## Restricted Changes Requiring Explicit Issue Authorization

The following changes require explicit authorization in the issue body:

* GitHub Actions workflow changes
* Deployment changes
* Infrastructure changes
* Database schema changes
* Secret-management configuration changes
* Release automation changes
* Terraform or other infrastructure-as-code changes
* Dockerfile or docker-compose.yml changes
* production configuration template changes

Examples of acceptable authorization sections:

```md
## Allowed restricted paths

- .github/workflows/**
- infra/**
- deploy/**
- Dockerfile
- docker-compose.yml

Reason:
- This issue is specifically about CI, deployment, or infrastructure.
```

If a restricted change is clearly authorized by the issue, Codex may implement it and explain the reason in the Draft PR.
If the issue does not authorize the required restricted path, Codex must stop and report the missing authorization.

<!--
非対話の GitHub Actions では Issue 本文の明示を承認として扱う。
許可がない restricted path は変更しない。
-->

Cargo.toml / Cargo.lock changes, including adding dependencies, are allowed when they are necessary to complete the
requested implementation. Codex must keep dependency changes minimal, avoid broad upgrades, and explain the reason in the
PR.

<!--
Cargo.toml / Cargo.lock の変更と依存追加は、依頼された実装に必要な場合は許可する。
ただし最小限にし、広範なアップグレードを避け、PR に理由を記載する。
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

Codex should avoid unless the issue explicitly requires a larger implementation:

* Large architectural changes
* Multi-directory refactors
* Renaming many files
* Rewriting core logic
* Broad dependency upgrades
* Multi-feature PRs

<!--
小規模修正・ログ改善・テスト追加・README 修正などを優先する。
Issue で明示されていない大規模設計変更や複数機能をまとめた PR は避ける。
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

## PR Size Guidance

Codex should keep changes as small as the issue reasonably allows.

The following are review guidance targets, not hard stop conditions:

```yaml id="u2k8cd"
preferred_max_changed_files: 10
preferred_max_added_lines: 300
preferred_max_deleted_lines: 150
```

If an issue explicitly asks for a broad implementation and lists the allowed paths, Codex may exceed these targets.
When exceeding them, Codex must keep the work focused on the issue, avoid unrelated refactors, and explain the larger
scope in the PR limitations or work-intent section.

<!--
変更ファイル数・追加行数・削除行数の目安は review しやすくするための推奨値であり、停止条件ではない。
Issue が広い実装と変更許可パスを明示している場合は、目安を超えてもよい。
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

* The workflow time budget is close to expiring and no coherent implementation can be completed
* The scope becomes unclear
* The required implementation would touch restricted paths that the issue did not explicitly authorize
* The required implementation would touch absolute forbidden files or secret values

<!--
時間切れが近い場合、不明確な要件、未承認の restricted path 変更、または絶対禁止対象の変更が必要な場合は停止する。
差分量だけを理由に停止しない。
-->

---

## Preferred Strategy

Codex should:

1. Read the issue carefully
2. Read all required AI documentation
3. Limit work to baseline paths and issue-authorized restricted paths
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
