## Context

どこを変更するか

## Goal

何を達成したいか

## Task Type

- [ ] Implementation
- [ ] Maintenance / Docs

## Non-goals

今回やらないこと

## Files or directories allowed to change

* src/commands/
* src/usecase/
* src/data/
* src/models/
* src/worker/
* src/services/
* tests/
* docs/

## Acceptance Criteria

* [ ] cargo test が通る
* [ ] cargo clippy --all-targets --all-features -- -D warnings が通る
* [ ] 既存機能を壊さない
* [ ] README が必要なら更新される
* [ ] 実装依頼の場合、`.codex` やドキュメントだけで完了扱いにしない
* [ ] 実装依頼の場合、`src/**` または `tests/**` に関連差分が入る

## Similar existing implementation

src/commands/help.rs
