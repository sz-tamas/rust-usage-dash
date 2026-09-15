# Project Skills

## Add a provider integration

Use this when adding a usage source such as OpenAI, Neon, Upstash, or Resend.

1. Add an adapter in `src/providers/` implementing the shared `Provider` trait.
2. Receive the resolved credential only as the `secret` argument; never add credential fields to a model, form, database query, or HTTP response.
3. Request only the provider's usage or billing endpoint and convert its response into normalized `Metric` values inside a `UsageSnapshot`.
4. Return concise, credential-free errors. Do not log the request, response body, or authentication details.
5. Register the adapter in `ProviderRegistry` and add focused tests using fixture data that contains no real secrets.

## Modify the dashboard UI

Use Askama templates in `templates/` and Tailwind classes in the markup. Prefer an Axum route that returns a fragment plus HTMX attributes (`hx-get`, `hx-post`, `hx-target`, `hx-swap`) over adding client-side state or a JSON endpoint.

After template or style changes, run:

```bash
mise run css:build
```

The generated CSS is ignored; it is a local build artifact.

## Work with local configuration

`mise.toml` is the project entrypoint. Keep non-secret developer settings there, including `USAGE_DASH_PORT`. Never place API keys, service account JSON, or provider tokens in mise configuration or `.env` files.

## Verify changes

Run:

```bash
mise run check
```

For changes that start the app, use `mise run start` and visit the configured localhost port. Do not bind to public interfaces to simplify testing.
