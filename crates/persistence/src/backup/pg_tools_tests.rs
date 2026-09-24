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
