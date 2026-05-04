You are implementing a GitHub issue for this repository.

Before making changes, read:
- AGENTS.md
- docs/ai/SKILLS.md
- docs/ai/ARCHITECTURE.md
- docs/ai/TESTING.md
- docs/ai/DECISIONS.md

Issue title:
[🏗️ 雑用] 誕生日リマインド機能の要件整理

Issue body:
## Context

親 Issue: #14

誕生日未登録ユーザー向けリマインド機能を分割実装する前に、アプリ側で確定すべき仕様を整理する。

## Goal

通知対象判定、通知頻度、停止条件、再開条件の仕様を文章として確定する。

## Task Type

- [ ] Implementation
- [x] Maintenance / Docs

## Non-goals

- コード実装
- DB schema 変更
- インフラ設定

## Files or directories allowed to change

- docs/
- README.md

## Acceptance Criteria

- [ ] #14 の要件から実装に必要な仕様が整理されている
- [ ] 実装依頼に必要な前提が文章化されている
- [ ] 未確定事項があれば明示されている

## Notes

- 親 Issue: #14


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
