You are implementing a GitHub issue for this repository.

Before making changes, read:
- AGENTS.md
- docs/ai/SKILLS.md
- docs/ai/ARCHITECTURE.md
- docs/ai/TESTING.md
- docs/ai/DECISIONS.md

Follow the active issue body as the source of truth for scope.

Rules:
- Follow AGENTS.md.
- If the issue explicitly allows scope overrides, follow the issue scope.
- Treat explicit issue sections such as "Allowed restricted paths", "Scope Override", or "Files or directories allowed to change" as human authorization for those paths.
- If the issue explicitly defines a broad implementation, do not stop solely because the work is large.
- Restricted paths such as workflows, deploy, infra, Dockerfile, docker-compose.yml, database schema, release automation, and infrastructure-as-code may be changed only when the issue explicitly authorizes them.
- Never commit secrets, .env files, real tokens, passwords, private keys, generated private backups, game saves, player data, or private logs.
- Write PR title and body in Japanese.
- Run cargo fmt --check, cargo clippy --all-targets --all-features -- -D warnings, cargo test when possible.
- If fmt, clippy, or test fails, record the failure and continue.
- Do not stop for separate approval in non-interactive automation when the issue already grants the required authorization.
- Stop if a required restricted path is not authorized by the issue.
- Keep changes focused on the issue only.
