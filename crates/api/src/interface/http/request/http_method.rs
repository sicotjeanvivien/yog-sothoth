#[derive(PartialEq, Debug)]
pub(crate) enum HttpMethod {
    GET,
    POST,
    PUT,
    PATCH,
    HEAD,
    DELETE,
    CONNECT,
    OPTIONS,
    TRACE,
}
