You are implementing a GitHub issue for this repository.

Before making changes, read:
- AGENTS.md
- docs/ai/SKILLS.md
- docs/ai/ARCHITECTURE.md
- docs/ai/TESTING.md
- docs/ai/DECISIONS.md

Issue title:
[🏗️ 雑用] 誕生日未登録ユーザー向けリマインド機能の要件整理

Issue body:
## Context

現在のDiscord botには誕生日登録機能があるが、未登録ユーザーに対するフォローがない状態。

そのため、誕生日未登録ユーザーに対して適切なタイミングでリマインドし、
登録を促す仕組みを追加したい。

---

## Goal

誕生日未登録ユーザーに対して、Bot専用チャンネルでメンション付きのリマインドを行い、
登録コマンドへの導線を提供する。

また、過度な通知を防ぐために以下を実現する：

* 段階的なリマインド頻度（指数バックオフ）
* 初回リマインドの遅延（UX改善）
* ユーザー単位での通知停止機能
* 低コスト運用（Cloud Run考慮）
* スパムにならない通知設計

---

## Non-goals

* 誕生日登録機能自体の改修
* UI（モーダル等）の追加
* エフェメラルメッセージの導入（※一部例外あり）
* 大規模サーバー向けの最適化

---

## Scope Override

この Issue に限り、以下の変更を許可する：

* ユーザーテーブルへのカラム追加
* 定期実行処理の追加
* メッセージイベントハンドリングの拡張
* Botチャンネル投稿処理の追加

---

## Data Model（追加カラム）

ユーザーテーブルに以下を追加する：

* last_active_at: Timestamp
* last_reminded_at: Timestamp | null
* next_remind_at: Timestamp | null
* remind_count: integer
* is_remind_opt_out: boolean

---

## Definitions

### アクティブユーザー

以下を満たすユーザー：

* 直近7日以内に以下のいずれかを行った

  * メッセージ送信
  * スラッシュコマンド実行

---

## Reminder Logic

### 初回

* 初回アクティブ時：

  * next_remind_at = last_active_at + 1日

---

### バックオフ（前回基準）

* 1回目: 1日後
* 2回目: 3日後
* 3回目: 7日後
* 4回目: 14日後
* 5回目: 30日後（最終）

※ remind_count >= 5 の場合は送信しない

---

### 送信条件

すべて満たす場合のみ送信：

* 誕生日未登録
* is_remind_opt_out = false
* remind_count < 5
* last_active_at within 7 days
* now >= next_remind_at
* now - last_reminded_at >= 24h

---

### 送信後更新

* last_reminded_at 更新
* remind_count += 1
* next_remind_at 再計算

---

## Notification

### 投稿仕様

* Bot専用チャンネルに投稿
* 1ユーザー1投稿（メンション付き）
* 投稿間に1〜3秒のディレイ

### メッセージ内容

まだ誕生日が登録されていないのだ！
よければ `/birth signup` から登録してほしいのだ！

通知が不要な場合は下のボタンから止められるのだ

---

### トーン

* 「〜のだ」で統一

---

## Stop / Resume

### 停止

* ボタン押下：

  * is_remind_opt_out = true
  * エフェメラル返信：「通知を停止したのだ！」
  * ボタンは無効化

---

### 権限制御

* 本人のみ操作可能
* 他人操作時：

  * エフェメラル返信：
    「この操作は自分のリマインドにのみ使えるのだ」

---

### 再開

* `/birth remind resume`

実行時：

* is_remind_opt_out = false

* next_remind_at = now + 1日

* エフェメラル返信：
  「リマインドを再開したのだ！」

---

## Trigger

### イベント

* メッセージ送信
* スラッシュコマンド

処理：

* last_active_at 更新
* if now < next_remind_at → return
* 条件満たせば送信

---

### スキャン

* 7日に1回
* last_active_at within 7 days

---

## Performance

* next_remind_at による early return
* 短期 in-memory cache

---

## Scheduled Execution (GCP)

定期スキャンは GCP Cloud Scheduler を使用して実行する。

### 構成

- Cloud Scheduler から HTTP リクエストで Cloud Run を起動
- 専用エンドポイントを用意する（例：`/internal/reminder/scan`）

---

### 実行仕様

- 実行頻度：7日に1回
- HTTP Method: POST
- 認証：
  - 内部利用のみ（認証ヘッダ or IAM を使用）
  - 外部から直接実行できないようにする

---

### エンドポイント仕様

- `/internal/reminder/scan`

処理内容：

- アクティブユーザー（7日以内）を取得
- リマインド条件を満たすユーザーを抽出
- 順次リマインド送信（ディレイあり）

---

### 制約

- スキャン処理は**冪等であること**
  - 同じリクエストが複数回実行されても問題ない設計
- next_remind_at によって重複送信を防ぐ

---

### 注意

- アプリ内部で cron は使用しない
- スケジューリングは Cloud Scheduler に完全委譲する

---

## Channel

環境変数：

```
BOT_CHANNEL_ID
```

セットアップ：

1. 「ずんだぼっと」チャンネル作成
2. ID取得
3. 環境変数設定

---

## Safety

* 24時間以内の再送禁止
* 再起動でスパム発生しない
* スパム的投稿を防ぐ（ディレイ）

---

## Implementation Guide

以下の順序で実装すること：

1. DB migration追加
2. Userモデル更新
3. アクティブ更新処理追加
4. リマインド判定ロジック実装
5. 投稿処理実装（ボタン付き）
6. ボタンハンドラ実装
7. resumeコマンド追加
8. 定期スキャン処理追加

---

## Expected Touch Points

* メッセージイベントハンドラ
* スラッシュコマンドハンドラ
* DBアクセス層
* 投稿処理
* ボタンinteraction処理
* スケジューラ

---

## Verification

以下を確認：

1. 未登録ユーザーが発言
   → 1日後に通知

2. 5回で停止

3. 停止ボタン
   → 通知されなくなる

4. resume実行
   → 再開（即時送信されない）

5. 複数ユーザー
   → スパムにならない

---

## Acceptance Criteria

* [ ] 未登録ユーザー判定ができる
* [ ] アクティブ条件が正しく動作
* [ ] リマインド送信される
* [ ] 5回で停止
* [ ] 停止・再開が動作
* [ ] 他人操作不可
* [ ] スパムにならない
* [ ] 既存機能に影響なし
* [ ] `cargo fmt` / `clippy` / `test` が通る

---

## Notes

* 通知は通常メッセージ
* スキャンは補助、イベントがメイン


Detected issue kind:
chore

Rules:
- Follow AGENTS.md
- If the issue explicitly allows scope overrides, follow the issue scope
- Do not modify forbidden files
- For implementation issues, do not finish with only .codex, prompt, or documentation changes
- For implementation issues, include at least one behavior-related change under src/** and validate it in tests/** when possible
- If repository policy blocks the required code path, stop and report the blocked path clearly
- Do not touch workflows, deploy, infra, or secrets unless the issue explicitly allows it
- Write PR title and body in Japanese
- Run cargo fmt --check, cargo clippy --all-targets --all-features -- -D warnings, cargo test
- Stop if approval is required
- Keep changes focused on the issue only
