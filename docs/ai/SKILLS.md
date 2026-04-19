# SKILLS.md

## Rust

* Prefer small functions
* Prefer explicit types over complex inference
* Avoid unnecessary trait abstractions
* Avoid macros unless already used in the repository
* Prefer anyhow for error propagation if already used
* Prefer Result<T, E> over panic
* Do not use unwrap unless existing code already uses it
* Prefer existing helper functions before adding new utilities

<!--
複雑な抽象化や不要な macro は避ける。
panic より Result を優先する。
unwrap は既存実装に合わせる。
-->

## Discord Bot

* Reuse existing command patterns
* Keep command handlers small
* Put shared logic under services if needed
* Prefer descriptive error messages for Discord replies

<!--
既存コマンド実装を流用する。
共通処理は services に寄せる。
-->

## Dependencies

* Avoid adding new dependencies
* Reuse existing crates where possible
* Any new dependency requires human approval

<!--
依存追加は禁止寄り。
既存 crate を優先する。
-->

## Logging

* Use existing logging macros
* Avoid excessive debug logs
* Error logs should include enough context

<!--
ログは既存スタイルに合わせる。
-->

## Tests

* Add tests for new behavior
* Prefer unit tests over integration tests for small changes
* Do not remove existing tests

<!--
新しい挙動には必ずテストを追加する。
-->
