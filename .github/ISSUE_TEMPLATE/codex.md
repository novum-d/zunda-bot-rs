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
* README.md
* docs/
* Cargo.toml
* Cargo.lock

## Allowed restricted paths

通常は空欄にする。以下の restricted path を変更する必要がある場合だけ、対象を明示する。

- [ ] .github/workflows/**
- [ ] infra/**
- [ ] deploy/**
- [ ] Dockerfile
- [ ] docker-compose.yml
- [ ] database schema / migrations
- [ ] release automation
- [ ] production configuration templates

Reason:

* なし

## Absolute forbidden changes

* `.env` / `.env.*` / `secrets/**`
* real tokens, passwords, private keys, credentials, or production secret values
* generated private backups, game saves, player data, or private logs

## Acceptance Criteria

* [ ] cargo test が通る
* [ ] cargo clippy --all-targets --all-features -- -D warnings が通る
* [ ] 既存機能を壊さない
* [ ] README が必要なら更新される
* [ ] 実装依頼の場合、`.codex` やドキュメントだけで完了扱いにしない
* [ ] 実装依頼の場合、`src/**` または `tests/**` に関連差分が入る
* [ ] restricted path を変更した場合、Issue の `Allowed restricted paths` に明示されている

## Similar existing implementation

src/commands/help.rs
