//! The guard on `.expose()`.
//!
//! The types in `secret` make a secret unprintable; `expose` is the one door
//! out, and this test is what keeps that door where it belongs. The rule it
//! enforces is not "call `expose` sparingly" — that is a habit, and a habit
//! applied at seven sites out of nine is what produced this ticket. The rule
//! is: **every exposure sits on the line that consumes the secret** — a
//! `connect`, a request builder, a client constructor owned by a third party.
//!
//! A new exposure anywhere else fails here, with the file named. Adding a
//! legitimate one means adding it to [`ALLOWED`] *with its reason* — writing
//! the reason is the point of the exercise, and a site whose reason cannot be
//! written is a site that should not exist.
//!
//! ⚠️ This guard was itself put in failure before being trusted: an exposure
//! added outside the list, and a count raised on a listed file, both turn it
//! red. A guard nobody has seen fail is a guard nobody knows works.

use std::{fs, path::Path};

/// Every file allowed to expose a secret, how many times, and why.
///
/// The count matters as much as the file: a second `.expose()` slipped into a
/// file that already had one would otherwise ride in unnoticed.
const ALLOWED: &[(&str, usize, &str)] = &[
    (
        "crates/api/src/bootstrap/app_state.rs",
        1,
        "sqlx owns the pool — the URL is the argument of `Database::connect`",
    ),
    (
        "crates/context/src/bootstrap/daemon.rs",
        1,
        "`init_db` takes the wrapped URL and exposes it at `Database::connect`",
    ),
    (
        "crates/context/src/providers/helius_das.rs",
        1,
        "reqwest owns the request — the URL is the argument of `.post`",
    ),
    (
        "crates/context/src/providers/jupiter_price.rs",
        1,
        "the key is the value of the `x-api-key` header, built on this line",
    ),
    (
        "crates/context/src/providers/solana_account.rs",
        1,
        "reqwest owns the request — the URL is the argument of `.post`",
    ),
    (
        "crates/indexer/src/application/workers/subscription.rs",
        1,
        "solana-pubsub-client owns the socket — argument of `PubsubClient::new`",
    ),
    (
        "crates/indexer/src/bootstrap/daemon.rs",
        2,
        "`init_db` at `Database::connect`, and `RpcClient::new`, whose type \
         belongs to solana-rpc-client",
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
];

/// Repository root, reached from this crate's manifest directory.
fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/bootstrap sits two levels below the workspace root")
}

/// Count the `.expose()` calls in one file, ignoring line comments.
///
/// Comments are skipped so that *writing about* the rule — as the daemons and
/// this module do — never trips it. A `.expose()` inside a block comment or a
/// string literal would still count; neither exists in this workspace, and a
/// guard that needed a Rust parser to be right would cost more than it saves.
fn count_exposures(source: &str) -> usize {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .map(|line| line.matches(".expose()").count())
        .sum()
}

/// Walk `crates/*/src`, collecting every file that exposes a secret.
///
/// Test sources are skipped: `secret_tests.rs` asserts on `expose` itself, and
/// a test that builds a client is not a production exposure path.
fn exposure_sites(root: &Path) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    let mut stack = vec![root.join("crates")];

    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("readable directory entry").path();
            if path.is_dir() {
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

/// The list is documentation as much as a guard, and an entry without a reason
/// documents nothing.
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

/// The guard must see what it claims to see, and ignore what it claims to
/// ignore. Without this, a counter that silently returned zero would make
/// every check above pass on an empty set.
#[test]
fn the_counter_sees_code_and_skips_comments() {
    assert_eq!(count_exposures("let a = x.expose();"), 1);
    assert_eq!(count_exposures("f(a.expose(), b.expose())"), 2);
    assert_eq!(count_exposures("// mentions .expose() in prose"), 0);
    assert_eq!(count_exposures("    /// doc mentioning .expose()"), 0);
    assert_eq!(count_exposures("let a = 1;"), 0);
}

/// The workspace really was walked. A typo in the root, a `crates` directory
/// that moved, or a filter that excluded everything would leave the main
/// assertion comparing two empty sets and passing for the wrong reason.
#[test]
fn the_walk_reaches_the_workspace() {
    let found = exposure_sites(workspace_root());
    assert!(
        found.len() >= 5,
        "the walk found {} file(s) — it is not reaching the sources",
        found.len()
    );
}
