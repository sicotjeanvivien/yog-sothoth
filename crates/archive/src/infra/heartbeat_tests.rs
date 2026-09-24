use super::*;

#[test]
fn fail_goes_into_the_path_of_a_plain_check_url() {
    let url = fail_url("https://hc-ping.com/0b7c5a1e-1111-2222-3333-444455556666").unwrap();
    assert_eq!(
        url.as_str(),
        "https://hc-ping.com/0b7c5a1e-1111-2222-3333-444455556666/fail"
    );
}

#[test]
fn fail_goes_into_the_path_of_a_slug_url_not_its_query() {
    let url = fail_url("https://hc-ping.com/PING-KEY/yog-archive?create=1").unwrap();
    assert_eq!(url.path(), "/PING-KEY/yog-archive/fail");
    assert_eq!(url.query(), Some("create=1"));
}

#[test]
fn a_trailing_slash_does_not_make_an_empty_segment() {
    let url = fail_url("https://hc-ping.com/abc/").unwrap();
    assert_eq!(url.path(), "/abc/fail");
}

#[test]
fn a_url_that_cannot_take_a_path_is_refused() {
    assert!(fail_url("not a url").is_none());
    assert!(fail_url("mailto:ops@example.com").is_none());
}
