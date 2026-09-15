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
- Provider API keys are not read from `.env`, configuration, or process environment variables. SQLite contains secret *references* and sanitized metrics only, never provider credential values.
- The browser receives provider metadata, refresh status, and normalized metrics only. It never receives a provider API key or a Secret Manager payload.
- Application Default Credentials (ADC) are intentionally stored locally by `gcloud` (normally in `~/.config/gcloud/application_default_credentials.json`). This is the Google authentication needed to read Secret Manager; it is not a provider API key.
- Every provider refresh obtains a fresh ADC token as needed, reads the configured Secret Manager version, and then makes the provider request. The dashboard does not cache provider API keys: a Secret Manager or provider-access failure fails that refresh rather than falling back to an older credential.
- The resolved provider key exists only in process memory for the request. Application-owned decoded key buffers use explicit zeroization after the request path completes; no key is written to a file, database, browser response, or log. Network/TLS libraries necessarily hold transient request-header buffers while sending the request.
- Logs contain only safe diagnostic metadata, such as an HTTP status or key-shape flags. They never contain a provider key, authorization header, Secret Manager payload, access token, or provider response body.

Before using production credentials, run `mise run check` and review IAM grants and provider-specific response handling.
