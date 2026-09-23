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

#[test]
fn the_password_leaves_the_url_and_is_decoded_for_pgpassword() {
    let secret = SecretUrl::for_tests("postgresql://yog_archive:p%40ss%2Fw0rd@db:5432/yog_sothoth");
    let connection = Connection::from_secret(&secret).unwrap();

    assert_eq!(
        connection.url,
        "postgresql://yog_archive@db:5432/yog_sothoth"
    );
    assert!(!connection.url.contains("p%40ss"));
    assert_eq!(connection.password.as_deref(), Some("p@ss/w0rd"));
}

#[test]
fn a_url_without_a_password_passes_none() {
    let secret = SecretUrl::for_tests("postgresql://yog_archive@db:5432/yog_sothoth");
    let connection = Connection::from_secret(&secret).unwrap();
    assert_eq!(connection.password, None);
}

#[test]
fn an_invalid_url_is_refused_without_quoting_it() {
    let secret = SecretUrl::for_tests("not a url with s3cret inside");
    let err = Connection::from_secret(&secret).err().unwrap();
    assert!(!err.to_string().contains("s3cret"), "{err}");
}

#[test]
fn tail_keeps_the_end_of_a_long_message() {
    let long = format!("{}END", "x".repeat(2000));
    let kept = tail(&long);
    assert!(kept.ends_with("END"));
    assert_eq!(kept.chars().count(), MESSAGE_TAIL + 1);
}

#[test]
fn a_socket_url_without_a_host_is_accepted() {
    let secret = SecretUrl::for_tests("postgresql:///yog_sothoth?host=/var/run/postgresql");
    let connection = Connection::from_secret(&secret).unwrap();
    assert_eq!(connection.password, None);
    assert_eq!(
        connection.url,
        "postgresql:///yog_sothoth?host=/var/run/postgresql"
    );
}
