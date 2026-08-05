# Homeabell

This is a mortgage amortization payment schedule tracker.

It allows you keep track of your mortgage payments and share with anyone you are paying with.

![example](img/example.png)

## Features

* Create multiple profiles for different mortgages
* Visualize breakdown of principal vs interest on each of your payments
* Track extra payments, both normal and recast
* Share your profile with another user

## Tech

Server-side Rust (Axum + Askama + HTMX). Locally it uses SQLite; in production it runs on **Cloudflare Containers** with **D1** (via a Worker DB RPC) and no Redis.

## Local Quick Start

* Copy `.env.sample` to `.env`
* `cargo run` (Rust toolchain required)

Sessions and auth rate limits are stored in SQLite (`sessions` / `rate_limits` tables).

### Optional: `wrangler dev` (Worker + Container via Docker)

Needs Docker (and on WSL, Docker reachable from that environment).

```bash
npm install
npx wrangler dev
```

Without `.dev.vars`, the container falls back to an in-container SQLite file and should listen on port 3000. Hit `http://localhost:8787`.

To seed a login under Wrangler, copy [`.dev.vars.example`](.dev.vars.example) → `.dev.vars` and set:

```bash
ALLOW_DEV_SEED_USERS=true
TEST_USER_EMAIL=test@example.com
TEST_USER_PASSWORD=choose-a-strong-local-password
```

Restart `wrangler dev` (or press `r` to rebuild/restart the container). The Worker forwards those vars into the container; Axum creates the user on boot if it does not exist.

To exercise D1 RPC locally instead, also set `INTERNAL_DB_SECRET` and `DB_RPC_URL=http://host.docker.internal:8787`. Apply local migrations with `npm run d1:migrate:local`.

## Cloudflare deploy (Containers + D1)

Prerequisites: Docker, Node.js, a Cloudflare account.

1. `npm install`
2. Create D1: `npx wrangler d1 create homeabell` and paste the `database_id` into [`wrangler.jsonc`](wrangler.jsonc)
3. Set secrets/vars:
   * `npx wrangler secret put INTERNAL_DB_SECRET` (or `cf` equivalent)
   * `vars.DB_RPC_URL` in `wrangler.jsonc` should be the public origin (`https://homeabell.com`)
4. Apply schema: `npm run d1:migrate:remote`
5. Deploy (builds the Rust image + Worker): `npm run deploy` or `cf deploy`

Custom Domains for `homeabell.com` and `www.homeabell.com` are configured in [`wrangler.jsonc`](wrangler.jsonc).
