use super::*;

#[test]
fn the_password_leaves_the_url_and_is_decoded_for_pgpassword() {
    let secret = SecretUrl::for_tests("postgresql://yog_archive:p%40ss%2Fw0rd@db:5432/yog_sothoth");
    let connection = DumpConnection::from_secret(&secret).unwrap();

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
    let connection = DumpConnection::from_secret(&secret).unwrap();
    assert_eq!(connection.password, None);
}

#[test]
fn an_invalid_url_is_refused_without_quoting_it() {
    let secret = SecretUrl::for_tests("not a url with s3cret inside");
    let err = DumpConnection::from_secret(&secret).err().unwrap();
    assert!(!err.to_string().contains("s3cret"), "{err}");
}

#[test]
fn a_socket_url_without_a_host_is_accepted() {
    let secret = SecretUrl::for_tests("postgresql:///yog_sothoth?host=/var/run/postgresql");
    let connection = DumpConnection::from_secret(&secret).unwrap();
    assert_eq!(connection.password, None);
    assert_eq!(
        connection.url,
        "postgresql:///yog_sothoth?host=/var/run/postgresql"
    );
}
