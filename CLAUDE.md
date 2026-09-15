# Project Instructions

Build a secure, local-first provider-usage dashboard.

## Required stack

- Rust backend using Axum
- Askama templates for pages and reusable HTML fragments
- HTMX for server-driven interactions
- Tailwind compiled by the standalone CLI; no Node.js, npm, `package.json`, or `node_modules`
- SQLite for provider metadata and sanitized usage snapshots
- Google Cloud Secret Manager with Application Default Credentials for credentials

## Security requirements

- Never write credential values to SQLite, files, logs, errors, or browser responses.
- Store only explicit Google Secret Manager version references.
- Keep secrets in memory only during collection.
- Never log authorization headers or raw provider responses that may contain sensitive data.
- Bind the web application only to `127.0.0.1`; read the port from `USAGE_DASH_PORT`.
- Keep Google IAM permissions limited to the required individual secrets.

## Code organization

- `src/secrets/`: secret resolver abstraction and GCP implementation
- `src/providers/`: provider adapters that produce normalized metrics
- `src/database.rs`: SQLite schema access and persistence
- `src/web/`: Axum routes and Askama rendering
- `templates/`: Tailwind-marked server-rendered pages and fragments
- `static/css/input.css`: Tailwind source; rebuild with `mise run css:build`

Use the same collection path for manual refresh and future scheduling. Implement provider-specific work behind the shared provider interface rather than branching inside web routes.

Before completing a change, run `mise run check`; run `mise run css:build` whenever templates or Tailwind source changes.
