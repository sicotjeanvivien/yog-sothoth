# yog-archive

The backup daemon. Every six hours it runs `pg_dump` against the database,
streams the dump into an S3-compatible bucket without writing it to disk,
checks that what it produced is a readable archive, and tells a dead man's
switch whether it succeeded.

It does **not** restore. A restore is rare, done by hand, and needs someone to
choose the dump and the target. The proven sequence, and what a dump does not
carry, are in [`persistence/README.md`](../persistence/README.md#backup-and-restore).

For the workspace-level picture, see [`crates/README.md`](../README.md).

---

## Layout

```
archive/src/
├── main.rs            ← bootstrap: tracing → Config → /metrics → Daemon → run
├── bootstrap/
│   ├── config.rs      ← Config::load (every ARCHIVE_* variable)
│   └── daemon.rs      ← builds the store, the heartbeat and the archiver, runs
│                         one dump every interval
├── archiver.rs        ← one run, ending in a RunOutcome that decides the signal
├── infra/             ← what a run calls outside the process
│   ├── heartbeat.rs   ← Healthchecks.io: success, or /fail with the reason
│   └── versions.rs    ← the server's versions, over a connection opened per run
└── metrics.rs
```

`pg_dump`, `pg_restore` and the split of the connection string are not here:
they are knowledge of the database, and live in `yog-persistence`'s `backup`
module ([`crates/persistence`](../persistence/README.md#the-backup-module)).
The archiver drives them: it reads the dump, streams it to the bucket, feeds
the check, and decides the `RunOutcome`.

## One run

1. Split the connection string for `pg_dump` (a socket URL with no host is
   accepted), connect, read the server's Postgres major and TimescaleDB version
   (`PgDatabaseInfo`, in `yog-persistence` — the binary writes no SQL), and
   close. The connection belongs to the run, not to the process: connected
   once at startup, a refusing database (a wrong password, a server down)
   stopped the process before it could signal anything — a silent crash loop
   under `restart: unless-stopped`. Now the same refusal ends the run in
   `refused`, and `/fail` carries the database's own words.
2. **Refuse** if `pg_dump --version` is not the server's major, naming both.
3. Open a multipart upload at
   `yog-sothoth/<UTC 2026-09-23T060000Z>_timescaledb-<version>.dump`, with the
   version also in the object's metadata (`timescaledb-version`,
   `postgres-major`). A dump restores only into that exact TimescaleDB
   version, and the object's name is what an operator reads first.
4. Run `pg_dump --format=custom --no-password`, the password in `PGPASSWORD`
   and never in the arguments, which any user of the host can read in
   `/proc/<pid>/cmdline`. Its output streams to the bucket in 8 MiB parts,
   two in flight at most.
5. Check the archive: `pg_dump` exited 0, **and** `pg_restore
   --file=/dev/null`, fed the whole stream alongside the upload, exits 0.
   Otherwise abort the upload. Restoring to a script file makes `pg_restore`
   read and decompress every data block: a dump cut in half, or with bytes
   zeroed in its data, fails it — measured — where `--list`, used by the
   first version, passed both. About 2 s for a 172 MB dump.
6. Complete the upload.

Each run ends in a `RunOutcome` — `archived`, `refused`, `dump_failed`,
`unreadable`, `store_failed` or `cancelled` — and the heartbeat is decided in a
single `match` with no catch-all. A new way to end cannot be added without
deciding what it signals.

### What "checked" means, and what it does not

The check proves the archive is complete and every data block decompresses. It
does **not** prove that the dump restores into a database — extensions,
versions, constraints: only a restore proves that. Run the procedure of `persistence/README.md` against a
recent dump from time to time — a backup whose restore has never been tried is
a hope.

### Why the upload is not read back

The bucket key is meant to be **write-only**, and a write-only key cannot
`HEAD` the object it just wrote, so the archiver trusts the completion of the
multipart upload — S3 assembles the object or refuses — instead of reading the
size back.

### ⚠️ A write-only key does not protect the backups on its own

It cannot delete an object, but it can **write over one**: on a bucket without
versioning, a `PUT` to an existing key replaces it. The keys are easy to guess —
a timestamp on a six-hour rhythm and a public TimescaleDB version — so a
compromised server could overwrite every dump of the window with junk, using
the very credentials the archiver holds. What makes the backups survive that is
the bucket's **versioning** (or Object Lock): an overwrite then adds a version
and the previous one stays, and a write-only key cannot delete versions. See
*Deploying*.

## Configuration

| Variable | Required | Meaning |
|---|---|---|
| `DATABASE_URL_ARCHIVE` | yes | Connection string of the `yog_archive` role (reads everything, writes nothing) |
| `ARCHIVE_STORE_URL` | yes | S3 endpoint, e.g. `https://s3.fr-par.scw.cloud`; `http://` accepted for a local MinIO |
| `ARCHIVE_STORE_BUCKET` | yes | Bucket name |
| `ARCHIVE_STORE_REGION` | yes | e.g. `fr-par` |
| `ARCHIVE_STORE_ACCESS_KEY` / `ARCHIVE_STORE_SECRET_KEY` | yes | A **write-only** key |
| `ARCHIVE_HEARTBEAT_URL` | yes | The check's ping URL, plain or slug form (`…/<ping-key>/<slug>?create=1`): `/fail` is pushed onto its path, never appended as text. Required: an archiver that fails in silence looks exactly like one that works |
| `ARCHIVE_INTERVAL_SECS` | no, `21600` | Time between dumps, at least 60 s (dumps are named to the second). The first one runs at startup |
| `ARCHIVE_METRICS_ADDR` | no, `0.0.0.0:9000` | Where `/metrics` listens. Change it on a host where 9000 is taken |
| `ARCHIVE_PG_DUMP` / `ARCHIVE_PG_RESTORE` | no, `pg_dump` / `pg_restore` | The client programs. They must be the server's major |

The image (`docker/backend.Dockerfile`, target `yog-archive`) starts from
`postgres:16-bookworm`, which carries both programs. **Moving the database to a
new Postgres major means moving this image with it** — the archiver refuses to
run otherwise, and says so.

## Deploying

- **Bucket versioning: on** (or Object Lock), for the reason given above —
  without it the write-only key still lets a compromised server overwrite
  every dump.
- **Bucket lifecycle rules**: expire objects under `yog-sothoth/` after
  **14 days** (56 dumps, ~9 GB at the September 2026 size), expire
  **noncurrent versions** after 14 days too — an overwritten dump stays
  recoverable that long, and versions do not pile up forever — and abort
  incomplete multipart uploads after **1 day**: a stop mid-dump aborts its
  upload, but a killed process cannot.
- **Key**: write-only on the bucket, no right to delete objects or versions.
  Retention belongs to the lifecycle rule, not to the archiver, for the same
  reason.
- **Healthchecks.io check**: period 6 hours, a grace of about an hour. A failed
  run pings `/fail` with the reason in the body; a missing ping raises the
  alarm on its own.
- **Role**: `yog_archive` is created by `yog-migrate setup-roles` with a
  `CHANGE_ME_…` password — set the real one with `\password yog_archive`.

## Stopping

SIGTERM or Ctrl-C mid-dump kills `pg_dump`, aborts the upload and signals
nothing: stopping is not failing, and the next start dumps at once. If that
takes longer than `SHUTDOWN_GRACE` (5 s), the process leaves anyway and the
lifecycle rule removes the incomplete upload.

The image inherits `STOPSIGNAL SIGINT` from its `postgres` base, so
`docker stop` delivers what the logs call Ctrl-C rather than SIGTERM. Both are
handled by the same `shutdown_signal`, so the difference is only in the log.

## Tests

```bash
cargo test -p yog-archive                      # DB-free: fake pg_dump / pg_restore, in-memory bucket
cargo test -p yog-persistence backup           # DB-free: the connection split, the version parse, a dropped dump is killed
cargo test -p yog-persistence --features integration-tests archive_role   # the role, under SET ROLE
```

The archiver tests run fake `pg_dump` / `pg_restore` shell scripts from a
temporary directory against an in-memory bucket and a heartbeat that records
what it is told. Every failure case asserts the reason, an empty bucket, and
exactly one failure signal carrying that reason. They run one at a time: a
script written then executed while another test forks can fail with
`ETXTBSY`.
