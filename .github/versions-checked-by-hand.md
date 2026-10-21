# Versions checked by hand

Dependabot (`dependabot.yml`) reads the manifests only. The versions below are pinned elsewhere, so it never
proposes them: check each one by hand when its monthly PRs arrive.

| What | Where | Moves with | Latest |
|---|---|---|---|
| Node in CI | `node-version:` in `workflows/web-quality.yml` (×4) and `workflows/crates.yml` | the `node:` image of `web/Dockerfile` | [nodejs.org](https://nodejs.org/en/about/previous-releases) |
| npm | `devEngines` in `web/package.json` | — | [npmjs.com](https://www.npmjs.com/package/npm) |
| TimescaleDB in CI | `services:` in `workflows/crates.yml` (×2) | the image of `docker-compose.yml` | [Docker Hub](https://hub.docker.com/r/timescale/timescaledb/tags) |
| sqlx-cli | `cargo install sqlx-cli --version` in `workflows/crates.yml` | the `sqlx` crate of `Cargo.toml` | [crates.io](https://crates.io/crates/sqlx-cli) |
| cargo-chef | `cargo install cargo-chef --version` in `docker/backend.Dockerfile` | — | [crates.io](https://crates.io/crates/cargo-chef) |
| Dockerfile syntax | `# syntax=` on line 1 of `docker/backend.Dockerfile` and `web/Dockerfile` | — | [Docker Hub](https://hub.docker.com/r/docker/dockerfile/tags) |
| Debian release | the `bookworm` of the `rust:`, `debian:` and `postgres:` images in `docker/backend.Dockerfile` | — | [debian.org](https://www.debian.org/releases/) |
| shellcheck | `SHELLCHECK_VERSION` and its `SHELLCHECK_SHA256` in `workflows/scripts.yml` | — | [GitHub](https://github.com/koalaman/shellcheck/releases) |
| actionlint | `go install …/actionlint@` in `workflows/scripts.yml` | — | [GitHub](https://github.com/rhysd/actionlint/releases) |
