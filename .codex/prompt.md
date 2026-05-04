You are implementing a GitHub issue for this repository.

Before making changes, read:
- AGENTS.md
- docs/ai/SKILLS.md
- docs/ai/ARCHITECTURE.md
- docs/ai/TESTING.md
- docs/ai/DECISIONS.md

Issue title:
[👔 ロジック実装] 誕生日未登録ユーザー向けリマインド機能の実装

Issue body:
# Task: Implement Birthday Reminder System (REQUIRED CODE CHANGE)

---

## ⚠️ Critical Instructions (MUST FOLLOW)

* This task REQUIRES actual code changes
* You MUST modify or create files under `src/**`
* If no code changes are made, the task is FAILED
* If implementation does not exist, CREATE new modules and wire them into the application
* Do NOT stop at planning or explanation

---

## Goal

Implement a birthday reminder system for users who have not registered their birthday.

---

## Data Model (REQUIRED)

Add the following fields to the user table:

* last_active_at: Timestamp
* last_reminded_at: Timestamp | null
* next_remind_at: Timestamp | null
* remind_count: integer
* is_remind_opt_out: boolean

If migration does not exist, CREATE it.

---

## Target Files (YOU MUST MODIFY)

* src/reminder/mod.rs (CREATE)
* src/reminder/service.rs (CREATE)
* src/handler/message.rs (UPDATE)
* src/handler/interaction.rs (UPDATE)
* src/db/user.rs or equivalent (UPDATE)

---

## Implementation Steps (EXECUTE IN ORDER)

### 1. Create Reminder Module

Create:

* src/reminder/mod.rs
* src/reminder/service.rs

---

### 2. Implement Reminder Logic

```rust
fn should_send_reminder(user: &User, now: DateTime) -> bool
```

Conditions:

* birthday is NULL
* is_remind_opt_out == false
* remind_count < 5
* last_active_at within 7 days
* now >= next_remind_at
* now - last_reminded_at >= 24h

---

### 3. Implement Backoff Logic

Backoff intervals:

* 1 → 1 day
* 2 → 3 days
* 3 → 7 days
* 4 → 14 days
* 5 → 30 days

---

### 4. Implement Reminder Sending

```rust
async fn send_reminder(user: &User)
```

Requirements:

* Send message to BOT_CHANNEL_ID
* Mention user
* Include `/birth signup`
* Include stop button

Message:

まだ誕生日が登録されていないのだ！
よければ `/birth signup` から登録してほしいのだ！

---

### 5. Update State After Sending

```rust
last_reminded_at = now
remind_count += 1
next_remind_at = calculate_next(remind_count)
```

---

### 6. Hook Into Message Event

In message handler:

```rust
update last_active_at

if now < next_remind_at:
    return

if should_send_reminder:
    send_reminder
```

---

### 7. Implement Stop Button

* Only target user can click
* If other user:
  return ephemeral error

On success:

```rust
is_remind_opt_out = true
```

Response:

通知を停止したのだ！

---

### 8. Implement Resume Command

Command:

```
/birth remind resume
```

Behavior:

```rust
is_remind_opt_out = false
next_remind_at = now + 1 day
```

Response:

リマインドを再開したのだ！

---

### 9. Implement Scheduled Scan Endpoint

Create:

```
POST /internal/reminder/scan
```

Behavior:

* Fetch users with last_active_at within 7 days
* Filter by reminder condition
* Send reminders sequentially
* Add 1–3 sec delay between sends

---

## Constraints

* Do NOT implement cron inside app
* Assume Cloud Scheduler triggers this endpoint
* Ensure idempotency (no duplicate sends)

---

## Environment Variable

```
BOT_CHANNEL_ID
```

---

## Safety Requirements

* Do not send multiple reminders within 24h
* Stop after 5 reminders
* Prevent spam with delay

---

## Verification (MUST PASS)

* User sends message → reminder scheduled
* Reminder is sent after delay
* Stops after 5 times
* Stop button works (only self)
* Resume works (no immediate send)
* Multiple users do not spam

---

## Output Requirements

* MUST modify or create files
* MUST produce a valid diff
* MUST compile
* MUST pass tests

---

## Definition of Done

* Reminder logic works end-to-end
* No duplicate notifications
* No CI failure (diff exists)


Detected issue kind:
logic

Rules:
- Follow AGENTS.md
- If the issue explicitly allows scope overrides, follow the issue scope
- Do not modify forbidden files
- For implementation issues, do not finish with only .codex, prompt, or documentation changes
- For implementation issues, include at least one behavior-related change under src/** and validate it in tests/** when possible
- If repository policy blocks the required code path, stop and report the blocked path clearly
- Do not touch workflows, deploy, infra, or secrets unless the issue explicitly allows it
- Write PR title and body in Japanese
- Run cargo fmt --check, cargo clippy --all-targets --all-features -- -D warnings, cargo test when possible
- If fmt, clippy, or test fails, record the failure and continue
- Stop if approval is required
- Keep changes focused on the issue only
