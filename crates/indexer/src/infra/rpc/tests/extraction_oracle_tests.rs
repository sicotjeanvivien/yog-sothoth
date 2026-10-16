//! Behaviour oracle: every mainnet DAMM v2 fixture's outcome, digested, against a committed
//! witness, so a refactoring is provable. `UPDATE_GOLDEN=1 cargo test -p yog-indexer
//! extraction_over_every_fixture` rewrites it, only when the change of behaviour is the point.
//! Mutation this is written against: shifting the inner-instruction groups by one.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use solana_transaction_status_client_types::EncodedConfirmedTransactionWithStatusMeta;
use yog_core::application::extraction::{
    EventExtractor, ExtractionOutcome, MeteoraDammV2, discriminator_hex,
};

use super::from_rpc;

/// Directory holding the mainnet transactions, and the witness they produce.
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../core/tests/fixtures/damm_v2")
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/golden/extraction.txt")
}

/// Every `*.json` directly inside the fixtures directory, sorted by name; non-recursive, so the
/// raw pool accounts of `accounts/` stay out.
fn fixture_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", dir.display()))
        .map(|entry| entry.expect("unreadable directory entry").path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "json"))
        .collect();

    files.sort();
    files
}

/// Render one transaction's outcome; an event's `Debug` carries its full position, `event_index`
/// included, which must not move.
fn render(outcome: &ExtractionOutcome) -> String {
    let mut out = String::new();

    writeln!(
        out,
        "  events={} unknown={} failures={}",
        outcome.events.len(),
        outcome.unknown.len(),
        outcome.failures.len()
    )
    .unwrap();

    for event in &outcome.events {
        writeln!(out, "  event {event:?}").unwrap();
    }
    for unknown in &outcome.unknown {
        writeln!(
            out,
            "  unknown protocol={:?} discriminator={}",
            unknown.protocol,
            discriminator_hex(&unknown.discriminator)
        )
        .unwrap();
    }
    for failure in &outcome.failures {
        writeln!(out, "  failure {failure}").unwrap();
    }

    out
}

/// Build the digest of the whole corpus; a fixture refused at transaction level is recorded,
/// not skipped, which would quietly shrink the corpus.
fn digest() -> String {
    let dir = fixtures_dir();
    let files = fixture_files(&dir);
    assert!(
        files.len() >= 28,
        "expected the 28 mainnet fixtures, found {} — did the corpus move?",
        files.len()
    );

    let extractor = MeteoraDammV2::new();
    let mut out = String::new();

    for file in files {
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        writeln!(out, "== {name}").unwrap();

        let raw = std::fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("failed to read fixture {name}: {e}"));

        match serde_json::from_str::<EncodedConfirmedTransactionWithStatusMeta>(&raw) {
            Err(e) => writeln!(out, "  PARSE ERROR {e}").unwrap(),
            Ok(tx) => {
                match from_rpc(&tx).and_then(|on_chain_tx| extractor.extract_events(&on_chain_tx)) {
                    Err(e) => writeln!(out, "  EXTRACTION ERROR {e}").unwrap(),
                    Ok(outcome) => out.push_str(&render(&outcome)),
                }
            }
        }
    }

    out
}

/// The corpus produces byte-for-byte what it produced before.
#[test]
fn extraction_over_every_fixture_is_unchanged() {
    let actual = digest();
    let path = golden_path();

    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).expect("failed to create golden dir");
        std::fs::write(&path, &actual).expect("failed to write the witness");
        panic!(
            "witness rewritten at {} — rerun without UPDATE_GOLDEN",
            path.display()
        );
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing witness {}: {e}\nGenerate it with UPDATE_GOLDEN=1",
            path.display()
        )
    });

    if actual == expected {
        return;
    }

    match first_difference(&expected, &actual) {
        Some((line, exp, act)) => panic!(
            "extraction behaviour changed at line {line} of {}\n  witness: {exp}\n  actual:  {act}",
            path.display()
        ),
        // Every line matches: the files differ in trailing whitespace alone, not in behaviour.
        None => panic!(
            "{} differs from the current output in trailing whitespace only — \
             extraction behaviour is unchanged. Restore the file's trailing \
             newline, or regenerate it with UPDATE_GOLDEN=1.",
            path.display()
        ),
    }
}

/// First differing line, so a failure names what moved; `None` when they agree line by line.
fn first_difference(expected: &str, actual: &str) -> Option<(usize, String, String)> {
    let mut exp_lines = expected.lines();
    let mut act_lines = actual.lines();

    for line in 1.. {
        match (exp_lines.next(), act_lines.next()) {
            (None, None) => return None,
            (e, a) if e != a => {
                return Some((
                    line,
                    e.unwrap_or("<end>").to_string(),
                    a.unwrap_or("<end>").to_string(),
                ));
            }
            _ => {}
        }
    }

    unreachable!()
}
