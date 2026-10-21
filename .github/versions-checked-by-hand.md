# Versions checked by hand

Dependabot (`dependabot.yml`) never proposes the versions below: check each one by hand when its weekly PRs arrive.

| What | Where | Moves with | Latest |
|---|---|---|---|
| Node in CI | `node-version:` in `.github/workflows/web-quality.yml` (×4) and `.github/workflows/crates.yml` | the `node:` image of `web/Dockerfile`, `@types/node` in `web/package.json` | [nodejs.org](https://nodejs.org/en/about/previous-releases) |
| npm | `devEngines` in `web/package.json` | — | [npmjs.com](https://www.npmjs.com/package/npm) |
| npm sub-dependencies | `web/package-lock.json`, beyond the packages `web/package.json` names | the packages that pull them in | `npm outdated --all` in `web/` |
| TypeScript 7 | the `ignore` of `typescript` in `.github/dependabot.yml`, which holds `web/package.json` on `6.*` | `typescript-eslint` (inside `eslint-config-next`) and `@hey-api/openapi-ts` — see [TypeScript 7](#typescript-7) | [typescript-eslint#10940](https://github.com/typescript-eslint/typescript-eslint/issues/10940) |
| TimescaleDB in CI | `services:` in `.github/workflows/crates.yml` (×2) | the image of `docker-compose.yml` | [Docker Hub](https://hub.docker.com/r/timescale/timescaledb/tags) |
| Postgres major of the database | the `-pg18` of `timescale/timescaledb` in `docker-compose.yml`, `docker-compose.prod.yml` and `.github/workflows/crates.yml` (×2) | the `postgres:` image of `docker/backend.Dockerfile`, the `timescaledb_pg18` volume of `docker-compose.yml` | [Docker Hub](https://hub.docker.com/r/timescale/timescaledb/tags) |
| sqlx-cli | `cargo install sqlx-cli --version` in `.github/workflows/crates.yml` | the `sqlx` crate of `Cargo.toml` | [crates.io](https://crates.io/crates/sqlx-cli) |
| cargo-chef | `cargo install cargo-chef --version` in `docker/backend.Dockerfile` | — | [crates.io](https://crates.io/crates/cargo-chef) |
| Dockerfile syntax | `# syntax=` on line 1 of `docker/backend.Dockerfile` and `web/Dockerfile` | — | [Docker Hub](https://hub.docker.com/r/docker/dockerfile/tags) |
| Debian release | the `bookworm` of the `rust:`, `debian:` and `postgres:` images in `docker/backend.Dockerfile` | — | [debian.org](https://www.debian.org/releases/) |
| shellcheck | `SHELLCHECK_VERSION` and its `SHELLCHECK_SHA256` in `.github/workflows/scripts.yml` | — | [GitHub](https://github.com/koalaman/shellcheck/releases) |
| actionlint | `go install …/actionlint@` in `.github/workflows/scripts.yml` | — | [GitHub](https://github.com/rhysd/actionlint/releases) |

## TypeScript 7

TypeScript 7.0 ships no compiler API, and two tools read it: typescript-eslint refuses to start (`npm run lint`), and openapi-ts crashes on `ts.SyntaxKind` (`npm run gen:api`). The `ignore` goes once both pass under TypeScript 7. Watch the issue above; when `npm view typescript-eslint peerDependencies.typescript` admits 7, run in `web/`:

```bash
npm install -D typescript@7 && npm run lint && npm run gen:api
git checkout package.json package-lock.json && npm ci
```

Both green: remove the `ignore`, and Dependabot proposes TypeScript 7 itself. A peer range alone is not enough — openapi-ts's admits 7 already, and it crashes.
