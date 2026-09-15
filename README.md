# Rust Usage Dashboard

A secure localhost-only dashboard for provider usage. It persists provider metadata and normalized usage snapshots in SQLite; provider credential values are fetched only when a refresh runs and are never stored in the database or sent to the browser.

## Prerequisites

- [mise](https://mise.jdx.dev/)
- Google Cloud CLI authenticated with Application Default Credentials: `gcloud auth application-default login`
- IAM access restricted to the specific Secret Manager secrets used by this dashboard

## Start

```bash
mise run install
mise run start
```

Open `http://127.0.0.1:3001`. Change `USAGE_DASH_PORT` in `mise.toml` if that port is in use.

The database defaults to `data/usage-dashboard.sqlite3`; override that non-secret path with `USAGE_DASH_DATABASE_PATH` if desired.

`mise run install` downloads the Tailwind standalone binary to `.tools/`. No `package.json` or `node_modules` is used. `mise run css:build` recompiles `static/css/output.css` from `static/css/input.css`.

For Rust development with automatic rebuilds and server restarts, run:

```bash
mise run dev
```

## Adding a provider

Enter a reference in this exact form:

```text
projects/PROJECT_ID/secrets/SECRET_ID/versions/VERSION
```

The current collectors implement Apify monthly usage and Resend email-quota usage. OpenAI, Neon, and Upstash can be saved now and have their adapters added without changing the secret or data model.

## Security boundary

- The server binds only to `127.0.0.1`; `USAGE_DASH_PORT` in `mise.toml` selects its port (currently: `3001`).
- SQLite contains secret *references*, never secret values.
- The GCP resolver obtains an ADC access token and calls the Secret Manager API directly. It never falls back to the separate `gcloud auth login` identity and does not log secret data.
- Provider requests use the secret only in memory and return sanitized metrics.

Before using production credentials, run `mise run check` and review IAM grants and provider-specific response handling.
