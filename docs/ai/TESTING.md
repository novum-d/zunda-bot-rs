# TESTING.md

## Purpose

This document explains the required testing process before Codex creates a Draft PR.

Codex must always run formatting, linting, and tests before creating a PR.

<!--
このファイルは Codex が Draft PR を作成する前に必要なテスト手順を定義する。
fmt、lint、test を必ず実行すること。
-->

---

## Required Commands

Codex must run the following commands before creating a PR:

```bash id="x2g7na"
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

<!--
上記コマンドがすべて成功しない限り PR を作成してはいけない。
-->

---

## Failure Rules

If any required command fails:

* Do not create a PR
* Stop the workflow
* Include the failure reason in logs
* Ask for human review if necessary

<!--
fmt、clippy、test のいずれかが失敗した場合は停止する。
-->

---

## Test Types

Preferred order of testing:

1. Unit tests
2. Small integration tests
3. Manual verification steps in PR description

<!--
小規模変更なら unit test を優先する。
integration test は必要最低限にする。
-->

---

## Rules for New Features

When adding a new feature:

* Add at least one test for the new behavior
* Cover success cases
* Cover expected failure cases if possible
* Reuse existing test patterns

<!--
新機能追加時は最低1つ以上のテストを追加する。
正常系だけでなく異常系も可能ならテストする。
-->

---

## Rules for Bug Fixes

When fixing a bug:

* Add a test that reproduces the bug if possible
* Ensure the bug no longer reproduces
* Avoid fixing bugs without test coverage unless impossible

<!--
バグ修正時は再現テストを追加する。
テストなしでの修正は避ける。
-->

---

## Forbidden Test Changes

Codex must never:

* Delete files under tests/
* Remove assertions
* Add #[ignore]
* Disable CI checks
* Reduce lint strictness
* Add allow(warnings)
* Remove existing test coverage
* Mark failing tests as ignored

<!--
失敗テストを隠すための変更は禁止。
-->

---

## Manual Verification

If automated testing is difficult, include manual verification steps in the PR.

Example:

```md id="z7r2tp"
## Manual Verification

1. Start the bot
2. Run the /ping command
3. Confirm that the response is returned successfully
```

<!--
自動テストが難しい場合は PR に手動確認手順を書く。
-->

---

## Expected PR Test Section

All PRs should include a test section.

Example:

```md id="n5d4ks"
## テスト結果

- cargo fmt --check
- cargo clippy --all-targets --all-features -- -D warnings
- cargo test
- 手動で /ping コマンドを確認
```

<!--
PR にテスト結果を必ず記載する。
-->
