use std::{fs, os::unix::fs::PermissionsExt, time::Duration};

use yog_bootstrap::SecretUrl;

use super::*;

#[test]
fn parse_major_reads_the_first_number_after_postgresql() {
    assert_eq!(parse_major("pg_dump (PostgreSQL) 16.14"), Some(16));
    assert_eq!(
        parse_major("pg_dump (PostgreSQL) 16.10 (Debian 16.10-1.pgdg120+1)\n"),
        Some(16)
    );
    assert_eq!(
        parse_major("pg_dump (PostgreSQL) 14.23 (Ubuntu 14.23-0ubuntu0.22.04.1)"),
        Some(14)
    );
    assert_eq!(parse_major("pg_dump 16.14"), None);
    assert_eq!(parse_major(""), None);
}

/// Whether `pid` has exited. A killed child stays a zombie until it is
/// reaped, and a zombie still answers `kill -0`: dead means gone from
/// `/proc`, or in state `Z`.
fn has_exited(pid: &str) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Err(_) => true,
        // `pid (comm) state …` — the state follows the closing parenthesis.
        Ok(stat) => stat
            .rsplit_once(')')
            .is_some_and(|(_, rest)| rest.trim_start().starts_with('Z')),
    }
}

#[tokio::test]
async fn dropping_a_dump_kills_pg_dump() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("pg_dump");
    let pid_file = dir.path().join("pid");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\necho $$ > {}\nprintf 'PGDMP-start'\nexec sleep 30\n",
            pid_file.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let tools = PgTools::new(script, "pg_restore".into());
    let connection =
        DumpConnection::from_secret(&SecretUrl::for_tests("postgresql://u@db/x")).unwrap();
    let mut dump = tools.start_dump(&connection).unwrap();
    let mut buf = [0u8; 64];
    let n = dump.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"PGDMP-start");
    let pid = fs::read_to_string(&pid_file).unwrap().trim().to_string();
    assert!(!has_exited(&pid), "pg_dump must be running before the drop");

    drop(dump);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !has_exited(&pid) {
        assert!(
            tokio::time::Instant::now() < deadline,
            "pg_dump {pid} still runs 5 s after its PgDump was dropped"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
