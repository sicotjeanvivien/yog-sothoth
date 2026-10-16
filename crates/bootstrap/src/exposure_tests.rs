//! The guard on `.expose()`: every exposure sits on the line that consumes the
//! secret. A new one fails here, with its file named, until it joins [`ALLOWED`]
//! with its reason.
//! Mutation: an exposure outside the list, or a count raised on a listed file.

use std::{fs, path::Path};

/// Every file allowed to expose a secret, how many times, and why.
/// ⚠️ Counted per file: an exposure moved within its file, into a `warn!`, passes.
const ALLOWED: &[(&str, usize, &str)] = &[
    (
        "crates/analytic/src/bootstrap/daemon/init.rs",
        1,
        "sqlx owns the pool — the URL is the argument of `Database::connect_with`",
    ),
    (
        "crates/api/src/bootstrap/app_state.rs",
        1,
        "sqlx owns the pool — the URL is the argument of `Database::connect_with`",
    ),
    (
        "crates/archive/src/infra/store.rs",
        2,
        "object_store owns the S3 signing — the access key id and the secret \
         are the arguments of `AmazonS3Builder`'s two setters",
    ),
    (
        "crates/archive/src/infra/versions.rs",
        1,
        "sqlx owns the pool — the URL is the argument of `Database::connect`, \
         for the connection each run opens to read the server's versions; its \
         refusal goes back through `SecretUrl::scrub`",
    ),
    (
        "crates/bootstrap/src/endpoint.rs",
        2,
        "the key's only consumers are its own two carriers — `Endpoint::url` and \
         `Endpoint::header`, where the halves meet, and the only places they do. \
         Both hand the result straight back into a secret type, so neither \
         exposure widens to a printable `String`",
    ),
    (
        "crates/bootstrap/src/heartbeat.rs",
        3,
        "reqwest owns the request — the check's URL is the argument of `.post` \
         for a success, and of `fail_url` for a failure, which pushes `fail` \
         onto its path and hands the result straight to `.post`; the third is \
         the same `fail_url` at startup, refusing a URL that cannot take it. \
         The errors reqwest builds from it go back through `SecretUrl::scrub`",
    ),
    (
        "crates/context/src/bootstrap/daemon.rs",
        1,
        "`init_db` takes the wrapped URL and exposes it at `Database::connect`",
    ),
    (
        "crates/context/src/infra/helius_das.rs",
        1,
        "reqwest owns the request — the URL is the argument of `.post`",
    ),
    (
        "crates/context/src/infra/jupiter_price.rs",
        1,
        "the key is the value of the `x-api-key` header, built on this line",
    ),
    (
        "crates/context/src/infra/solana_account.rs",
        1,
        "reqwest owns the request — the URL is the argument of `.post`",
    ),
    (
        "crates/indexer/src/bin/inspect_logs.rs",
        1,
        "tokio-tungstenite owns the socket — argument of `connect_async`",
    ),
    (
        "crates/indexer/src/infra/endpoint/credential.rs",
        2,
        "the two lines where a credential meets the client that will carry it, \
         and they are two because there are two carriers: the key becomes the \
         value of a header, and the URL becomes a WebSocket handshake request. \
         Both paths' listeners go through here rather than exposing their own — \
         which is why `subscription.rs` and `grpc/interceptor.rs` left this list \
         on 10 September 2026, and why the count is two rather than four",
    ),
    (
        "crates/indexer/src/infra/grpc/listener/connector.rs",
        1,
        "tonic owns the channel — the URL is the argument of \
         `Endpoint::from_shared`. Every error built from it on that path goes \
         back through `SecretUrl::scrub`, since tonic quotes the URI it was \
         handed",
    ),
    (
        "crates/indexer/src/bootstrap/daemon/init.rs",
        4,
        "`init_db` at `Database::connect`; `init_alarm_db` at \
         `Database::connect_with`, the same address exposed a second time on \
         purpose — the alarm reads under its own `statement_timeout` rather \
         than through the pool the index tasks are sized against; and \
         `RpcClient::new` **twice**, whose \
         type belongs to solana-rpc-client. The second one arrived on \
         10 September 2026 and is a widening this list should show rather than \
         hide: the ingestion source and the health reporter each open their own \
         client instead of sharing one. The alternative was the composition \
         root holding a client on behalf of one of the two ingestion paths. \
         ⚠️ Until 21 September 2026 the two sites exposed the *same* address, \
         which made the widening look like an accounting detail; they now read \
         two variables — `INGEST_TRANSACTION` and `NETWORK_STATUS` — and may \
         point at two providers, so this list covers two addresses, not one \
         twice",
    ),
    (
        "crates/persistence/src/backup/pg_tools.rs",
        1,
        "the process environment owns the password — it is the argument of \
         `command.env(\"PGPASSWORD\", …)`, which `pg_dump` reads; the URL it \
         receives as an argument (readable in `/proc/<pid>/cmdline`) comes \
         from `SecretUrl::split_password` and holds none",
    ),
    (
        "crates/persistence/src/bin/migrate.rs",
        1,
        "sqlx owns the pool — argument of `Database::connect_for_provisioning`",
    ),
    (
        "crates/signals/src/bootstrap/daemon.rs",
        1,
        "sqlx owns the pool — the URL is the argument of `Database::connect`",
    ),
    (
        "crates/signals/src/bootstrap/daemon/init.rs",
        1,
        "sqlx owns the pool — the URL is the argument of `Database::connect_with` \
         for the materialisation alarm's own pool. The same address as \
         `daemon.rs`'s, exposed a second time on purpose: the alarm reads under \
         its own `statement_timeout` rather than through the pool it watches \
         over",
    ),
];

/// Repository root, reached from this crate's manifest directory.
fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/bootstrap sits two levels below the workspace root")
}

/// Count the `.expose()` calls in one file, ignoring line comments.
/// ⚠️ Whole-line comments only: one in a trailing comment, a block comment or a string counts.
fn count_exposures(source: &str) -> usize {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .map(|line| line.matches(".expose()").count())
        .sum()
}

/// Walk `crates/`, collecting every file that exposes a secret; `*_tests.rs` and
/// `tests/` are skipped, a test not being a production exposure path.
fn exposure_sites(root: &Path) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    let mut stack = vec![root.join("crates")];

    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("readable directory entry").path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "tests") {
                    continue;
                }
                stack.push(path);
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if !name.ends_with(".rs") || name.ends_with("_tests.rs") {
                continue;
            }
            let source = fs::read_to_string(&path).expect("readable source file");
            let count = count_exposures(&source);
            if count > 0 {
                let relative = path
                    .strip_prefix(root)
                    .expect("path built from root")
                    .to_string_lossy()
                    .replace('\\', "/");
                found.push((relative, count));
            }
        }
    }
    found.sort();
    found
}

#[test]
fn every_exposure_site_is_a_known_point_of_consumption() {
    let root = workspace_root();
    let found = exposure_sites(root);

    let mut expected: Vec<(String, usize)> = ALLOWED
        .iter()
        .map(|(file, count, _)| ((*file).to_string(), *count))
        .collect();
    expected.sort();

    let unexpected: Vec<_> = found.iter().filter(|f| !expected.contains(f)).collect();
    let missing: Vec<_> = expected.iter().filter(|e| !found.contains(e)).collect();

    assert!(
        unexpected.is_empty() && missing.is_empty(),
        "the set of `.expose()` sites moved.\n\
         Unexpected (a secret escapes its type somewhere new): {unexpected:#?}\n\
         Missing (the list is stale, or a count dropped): {missing:#?}\n\
         Add a site to ALLOWED only with the reason it is a point of \
         consumption — see this module's docs."
    );
}

/// Mutation: an `ALLOWED` entry without a reason.
#[test]
fn every_allowed_site_carries_its_reason() {
    for (file, count, reason) in ALLOWED {
        assert!(*count > 0, "{file} is listed with a count of zero");
        assert!(
            reason.len() > 20,
            "{file} is listed without a real reason: {reason:?}"
        );
    }
}

/// Mutation: a counter that returns zero, or counts commented lines.
#[test]
fn the_counter_sees_code_and_skips_comments() {
    assert_eq!(count_exposures("let a = x.expose();"), 1);
    assert_eq!(count_exposures("f(a.expose(), b.expose())"), 2);
    assert_eq!(count_exposures("// mentions .expose() in prose"), 0);
    assert_eq!(count_exposures("    /// doc mentioning .expose()"), 0);
    assert_eq!(count_exposures("let a = 1;"), 0);
}

/// Mutation: a walk that finds nothing, comparing two empty sets.
#[test]
fn the_walk_reaches_the_workspace() {
    let found = exposure_sites(workspace_root());
    assert!(
        found.len() >= 5,
        "the walk found {} file(s) — it is not reaching the sources",
        found.len()
    );
}
