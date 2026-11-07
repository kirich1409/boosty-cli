## Context

`boosty-cli` is a local CLI for the Boosty DevAPI. It reads blog-scoped data with an application API key and user-scoped data with a user access token obtained through the authorization-code flow. Humans get readable Russian text. Agents and scripts pass a global `--json` flag and parse stdout. Code, comments, logs, and this file stay English.

The API contract is `devapi.md` and `authorization_code_flow.md`. Those files are the source of truth. Do not add endpoints, fields, or grants they do not describe. The unofficial Boosty web API (blog posts, comments, cookie sessions, `/v1/blog/{name}/...`) is a different API and is out of scope.

## Commands

From the repository root, after code changes:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

`cargo test` must pass with no network access and without the developer machine's Boosty credentials. `cargo build` produces the `boosty` binary. Invoke it through `cargo run --` while developing.

The crate is a single binary package at the repository root. Do not pass `--workspace` or a package selector.

## Testing

Add tests in the same change as the behavior they cover. A new DevAPI command needs:

- a decode test from a fixture shaped like the spec, with fake values
- a local HTTP test that asserts method, path, query, and exactly one auth header
- an assertion that stdout stays machine-parseable when `--json` is set

Also cover Basic-header encoding, `redirect_url` versus `redirect_uri`, refresh-token rotation, OAuth state mismatch, user denial, atomic token-file replacement, and passing `extra.nextOffset` through unchanged.

Point HTTP tests at a local server with the base-URL overrides below. Do not call Boosty from tests. Do not read `BOOSTY_API_KEY`, `BOOSTY_CLIENT_SECRET`, or the developer's token file from the environment in tests. Do not copy sample credentials out of the spec files into fixtures.

There is no coverage gate. Do not weaken or skip a test to make a red suite green.

## Project layout

Single Cargo binary crate, edition 2024. Navigation:

- `src/cli.rs` — clap command tree and the global `--json` flag
- `src/client.rs` — hosts, timeouts, and the only place that sets HTTP auth headers
- `src/auth.rs` — authorize URL, code exchange, refresh
- `src/config.rs` — environment variables and the token file
- `src/output.rs` — human text versus JSON, stdout versus stderr
- `src/api/` — wire models and one method per DevAPI resource
- `devapi.md`, `authorization_code_flow.md` — API contract
- `tests/fixtures/` — fake response bodies

Dependency direction: `cli` may call `auth`, `api`, `output`, and `config`. `api` and `auth` may call `client`. `client` does not depend on `cli`. Commands do not assemble URLs or auth headers themselves.

Do not extract a library crate, an MCP server, or an agent skill until an in-process caller is an explicit task. The product is the `boosty` binary.

## Dev environment

Run Cargo from the repository root. Do not introduce Gradle, npm, or a second language toolchain.

TLS uses `reqwest` with the rustls backend and default features disabled. Do not enable native-tls or pull in OpenSSL. Use one tokio runtime at the process edge. Do not call `reqwest::blocking` from that runtime.

Edition is 2024. Format with rustfmt defaults. Clippy warnings are errors, including tests.

Hosts, unless a test override is set:

- authorize: `https://boosty.to/app/oauth2/authorize`
- token: `https://api.boosty.to/oauth/server/token`
- DevAPI: `https://devapi.boosty.to/v1`

Overrides exist for tests and local stubs only: `BOOSTY_AUTHORIZE_URL`, `BOOSTY_TOKEN_URL`, `BOOSTY_DEVAPI_BASE`. Production defaults live in `src/client.rs`, not in call sites.

## Architecture

Credentials:

- `BOOSTY_API_KEY` — application secret sent as `X-Api-Key`. Required for `subscription-levels`, `subscriptions`, and `subscribers`. Responses are relative to the application's blog. There is no blog-id argument.
- `BOOSTY_CLIENT_ID` and `BOOSTY_CLIENT_SECRET` — used only to build the token endpoint's `Authorization: Basic` value, `base64(client_id + ":" + secret)` with the standard base64 alphabet.
- `BOOSTY_REDIRECT_URL` — the registered redirect URL. The same string is sent as `redirect_url` on authorize and as `redirect_uri` on the token request.
- User tokens live in a file, not in the environment. Default path is `$XDG_CONFIG_HOME/boosty-cli/tokens.json`, or `~/.config/boosty-cli/tokens.json` when `XDG_CONFIG_HOME` is unset, including on macOS. Override with `BOOSTY_TOKEN_PATH`. Directory mode `0700`, file mode `0600`.

Never accept an API key, client secret, access token, or refresh token as a flag or positional argument.

The token file stores `access_token`, `refresh_token`, and absolute `expires_at` (unix seconds). It does not store the client secret or API key. Compute `expires_at` from `expires_in` at write time. Refresh when the access token expires within 60 seconds. A successful refresh returns a new refresh token; persist both new tokens with a temp-file rename before the command reports success. If refresh or the rename fails, leave the previous file unchanged.

`auth login` is the only interactive command. Listen on the host and port parsed from `BOOSTY_REDIRECT_URL`, and accept the callback only on that URL's path. Do not hardcode `127.0.0.1` or any other address. The registered URL is a localhost redirect, so refuse a non-loopback host instead of binding it. Do not choose an unregistered port. Generate a cryptographic `state` per attempt and reject a callback whose `state` differs. On `error` or `error_reason`, exit as an auth failure and do not call the token endpoint. Open the system browser; if that fails, print the authorize URL to stderr. Stop listening after a few minutes. Agents retrieve data with an API key or an existing token file; they do not run `auth login`.

Auth header for each call:

- token endpoint: Basic client credentials only
- `GET /v1/user`: `Authorization: Bearer` only
- `GET /v1/subscription_levels`, `/v1/subscriptions`, `/v1/subscribers`: `X-Api-Key` only

If the required credential is missing, fail. Do not substitute the other mode and do not send both headers. `boosty user` sends `Authorization: Bearer` only. The `X-Api-Key` alternative in the `/v1/user` example is not a second accepted mode.

Command surface:

- `boosty auth login`
- `boosty auth refresh`
- `boosty auth status`
- `boosty user`
- `boosty subscription-levels`
- `boosty subscriptions`
- `boosty subscribers [--is-free true|false] [--limit N] [--offset N]`

`--json` is global and works both before and after the subcommand. One invocation performs one request and, for subscribers, one page. Do not loop pages inside a command. The caller passes the returned `extra.nextOffset` back as `--offset`.

Exit codes: `0` success, `2` usage (leave clap's default alone), `3` missing or rejected credentials, user denial, state mismatch, or refresh failure, `4` remote API or transport failure, `5` local configuration or filesystem failure. On failure, leave stdout empty. With `--json`, write one stderr object `{"error":"...","kind":"usage"|"auth"|"api"|"io"}` with English `kind` and `error` — that stream is for agents. Without `--json`, write a short Russian message to stderr. Do not include tokens, secrets, or authorization codes in either stream.

On success, an API command writes the response body for `--json`, preserving the spec's field names. Key order is not part of the contract. `auth status` and a successful login print expiry and the token-file path, never the token values. An empty list is success.

Human stdout is a Russian view, not a JSON dump and not a column of raw field names. Group related rows. Show names before ids. Format a `price` string for reading (`RUB11.00` may be shown as `11.00 RUB`) without changing the JSON value. Render non-zero timestamps in the local timezone as a date and time; render `0` as «нет». Mark `isDeleted` as «удалён». Print `nextOffset` as a labeled line, `следующее смещение: N`, so the next command is obvious. Clap help, command descriptions, and flag docs are Russian too; do not leave the stock English help template.

HTTP calls set `Accept: application/json` and an explicit timeout of 30 seconds. Set a `boosty-cli/<version>` user agent. Parse the body as JSON even if the content type is missing. Ignore unknown JSON fields. Fail if a field required by the spec is missing. Keep `token_type`; send a bearer header only when it is `Bearer`.

## Code style

User-facing help, human stdout, and human stderr are Russian. `--json` field names and error `kind` values stay the English spec names. Spec files stay the Russian contract; do not translate them into a second API document. Do not translate wire identifiers in code.

Wire structs use the API's camelCase names in JSON. `price` and `payments` stay strings. Timestamps and ids are `i64`. Do not narrow ids to `i32`. Do not turn `0` timestamps into `1970-01-01` or into JSON `null`. Human output may render a `0` timestamp as `-`; `--json` keeps the integer.

`is_free` is the string `true` or `false`. Omit the query parameter when the flag is absent. Do not drop `isDeleted` levels or any other documented row unless a flag asks for that filter. There is no such flag yet.

Authorize uses the query name `redirect_url`. The token form uses `redirect_uri`. Keep both names. Token parameters go in an `application/x-www-form-urlencoded` body on `POST`, even though the spec heading says "Get параметры".

Do not put secrets, authorization codes, or full response bodies into `Display`, `Debug`, or log lines. A comment at the token-request call site should state that the `redirect_url` / `redirect_uri` split is intentional.

## Common workflows

Add a read command only after the method is in `devapi.md`:

1. Add the wire model under `src/api/` and a client method that sets the one required header.
2. Register a subcommand in `src/cli.rs`.
3. Add a fixture and a local HTTP test.
4. Run the fmt, clippy, and test commands above.

One `subscribers` invocation is one page. Print `nextOffset` when the response includes it, and pass that value back as `--offset` for another page. The spec does not define the last page. Do not treat a missing `nextOffset`, a short page, or `nextOffset` equal to the offset just sent as an end signal.

Refresh is automatic before a bearer call inside the skew window, and manual via `boosty auth refresh`.

When the live API disagrees with the spec, update the spec file and the matching pitfall here in the same change. Do not special-case an undocumented field in the client.

## Known pitfalls

- `redirect_url` and `redirect_uri` are different parameter names for the same registered URL. Renaming either one breaks the flow. The value must match the registration character for character.
- Refresh rotates `refresh_token`. Persisting the old refresh token after a successful refresh invalidates the session. A non-atomic write can destroy the only copy.
- `extra.nextOffset` is a cursor to pass back as `offset`. The spec example is not `offset + limit`. Do not synthesize it, and do not invent an end-of-list check.
- API-key calls are scoped to the application blog. Do not add a blog name or blog id to make them look like the web API.
- The published `boosty_api` crate models the web API, not DevAPI: `price` is a number, deletion is `deleted`, and levels are wrapped in `data`. Do not reuse those models.
- The spec files contain sample client secrets, API keys, and tokens. Treat those values as secrets. Do not copy them into code, tests, fixtures, logs, or this file.
- `endTime` and `nextPayTime` are `0` in the spec when unset. That is not a parse failure.
- `price` looks like `RUB11.00`: a currency prefix plus an amount, one string. Do not parse it as a number.
- Subscriber payloads include email addresses. They belong in the command output because the API returns them. They do not belong in logs.
- Sending Basic auth to DevAPI, or `X-Api-Key` to the token endpoint, will not work. The header families are not interchangeable.
- Clap owns exit code `2`. Do not reuse it for auth failures.
- With `--json`, any log line on stdout breaks the caller. Warnings and the authorize URL go to stderr.
- The token path stays under `~/.config/boosty-cli` on macOS so agents have one path. Do not switch it to Application Support.
- Do not commit the token file, `.env`, or a filled `BOOSTY_TOKEN_PATH`. Gitignore those paths.

## Maintaining this file

Keep this file up to date when you change commands, layout, auth, environment variables, exit codes, or an API quirk. Add a rule only when it is specific to this repository and stable. Remove guidance that the code no longer follows.

Do not record changelogs, dated decisions, feature inventories, dependency versions, or generic Rust, Cargo, or HTTP tutorials. Do not paste secrets or live response bodies here. If a convention is obvious from `Cargo.toml` or the spec files, leave it out.
