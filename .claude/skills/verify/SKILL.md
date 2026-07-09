---
name: verify
description: How to build, launch and drive Yog-Sothoth locally to verify a change end-to-end (API + web dashboard), including the WSL2 browser-driving recipe.
---

# Verify a change end-to-end (Yog-Sothoth)

## Launch the stack (native, fastest)

```bash
# 1. Postgres (data volume persists; ~750 pools + past signals in local dev)
docker compose up -d postgres          # WSL2: daemon may need `sudo service docker start` (user-run)

# 2. API — .env has CRLF line endings, strip them when exporting
env $(tr -d '\r' < .env | grep -v '^#' | grep -E '^[A-Z_]+=' | xargs) cargo run -p yog-api
curl -s http://127.0.0.1:5000/healthz  # wait for 200

# 3. Web (web/.env.local already points at :5000)
cd web && npm run dev                  # http://localhost:3000, dashboard routes 307-redirect (curl -L)
```

## Seed observable data

Signals older than 24h don't feed the pools-list indicator. Insert fresh ones
as the admin role, tagged for cleanup:

```sql
INSERT INTO signals (detector, protocol, pool_address, severity, value, threshold, message, triggered_at)
VALUES ('flow_imbalance','meteora_damm_v2','<addr from GET /api/pools>','warning',0.75,0.6,'VERIFY_TEST', now() - interval '2 hours');
-- afterwards: DELETE FROM signals WHERE message='VERIFY_TEST';
```

`psql "postgresql://yog:yog@localhost:5433/yog_sothoth"` (host port is **5433**).

## Drive the GUI from WSL2 (no Linux browser installed)

- **Static screenshot**: Windows Edge works headless against the WSL service:
  `"/mnt/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe" --headless=new --screenshot="C:\\Users\\<user>\\AppData\\Local\\Temp\\x.png" http://localhost:3000/en/pools`
  (write the PNG to a Windows path; read it back via /mnt/c). Its CDP port binds
  to Windows-localhost only — **not reachable from WSL**, so no interaction this way.
- **Interactions (hover/click/keyboard)**: install Playwright in the scratchpad
  (never in `web/` — package-lock desync breaks CI `npm ci`):
  ```bash
  cd <scratchpad> && npm init -y && npm i playwright && npx playwright install chromium-headless-shell
  # missing libasound.so.2 and no sudo? extract it locally:
  apt-get download libasound2 && dpkg -x *.deb extracted
  LD_LIBRARY_PATH=$PWD/extracted/usr/lib/x86_64-linux-gnu node drive.mjs
  ```

## Flows worth driving

- `/en/pools` and `/fr/pools` — table renders, locale strings (plural forms).
- Pools-list signal indicator: icon severity/color, hover popover, click →
  `?tab=alerts`, keyboard focus + Enter, and that clicking the rest of the row
  still opens the plain pool page (the row is one big `<Link>`).
- API directly: `curl -s localhost:5000/api/pools?limit=3 | jq` — check new
  wire fields on real data, not just fixtures.

## Gotchas

- SQLx: any new/changed `query!` needs `cd crates/persistence && cargo sqlx prepare`
  against the live DB before the workspace compiles offline.
- Local signals data is stale (last detector run date) — always check
  `SELECT max(triggered_at) FROM signals;` before assuming "no data" is a bug.
