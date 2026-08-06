# Mortgage Track

This is a mortgage amortization payment schedule tracker.

It allows you keep track of your mortgage payments and share with anyone you are paying with.

![example](img/example.png)

## Features

* Create multiple profiles for different mortgages
* Visualize breakdown of principal vs interest on each of your payments
* Track extra payments, both normal and recast
* Share your profile with another user

## Tech

Server side rendered (Axum + Askama + HTMX) with SQLite.

## Local Quick Start

* Copy `.env.sample` to `.env`
* `cargo run` (Rust toolchain required)

## Docker

```bash
docker build -t mortgagetrack .
docker run --rm -p 3000:3000 \
  -e SESSION_SECURE=false \
  -e DATABASE_URL=sqlite:/tmp/mortgage.db \
  mortgagetrack
```