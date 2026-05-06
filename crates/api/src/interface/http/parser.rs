use std::collections::HashMap;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::interface::{HttpError, HttpMethod, HttpRequest};

/// Taille initiale du buffer ; on agrandit si nécessaire.
const INITIAL_BUFFER_SIZE: usize = 4096;

pub(crate) async fn decode_request<R>(stream: &mut R) -> Result<HttpRequest, HttpError>
where
    R: AsyncRead + Unpin,
{
    // ── Lecture complète des headers (jusqu'à \r\n\r\n) ──────────────────────
    let raw = read_until_header_end(stream).await?;
    let raw_str = String::from_utf8(raw).map_err(|_| HttpError::InvalidEncoding)?;

    let mut lines = raw_str.lines();

    // ── 1️⃣  Ligne de requête ────────────────────────────────────────────────
    let request_line = lines.next().ok_or(HttpError::MalformedRequest)?;
    let mut parts = request_line.split_whitespace();

    let method = parse_method(parts.next().ok_or(HttpError::MalformedRequest)?)?;
    let raw_path = parts.next().ok_or(HttpError::MalformedRequest)?;
    let http_version = parts.next().ok_or(HttpError::MalformedRequest)?;

    // ── 2️⃣  Query params (/path?foo=bar&baz=qux) ────────────────────────────
    let (path, params) = parse_path_and_params(raw_path);

    // ── 3️⃣  Headers ─────────────────────────────────────────────────────────
    let mut headers: HashMap<String, String> = HashMap::new();
    for line in &mut lines {
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }

    // ── 4️⃣  Body (respecte Content-Length) ──────────────────────────────────
    let body = if let Some(len) = content_length(&headers) {
        if len == 0 {
            None
        } else {
            let mut buf = vec![0u8; len];
            stream
                .read_exact(&mut buf)
                .await
                .map_err(|_| HttpError::IoError)?;
            Some(String::from_utf8_lossy(&buf).into_owned())
        }
    } else {
        None
    };

    Ok(HttpRequest::new(
        method,
        path,
        params,
        http_version.to_string(),
        headers,
        body,
    ))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Lit octet par octet jusqu'à trouver la séquence de fin de headers HTTP.
async fn read_until_header_end<R>(stream: &mut R) -> Result<Vec<u8>, HttpError>
where
    R: AsyncRead + Unpin,
{
    let mut buf = Vec::with_capacity(INITIAL_BUFFER_SIZE);
    let mut byte = [0u8];

    loop {
        match stream.read(&mut byte).await {
            Ok(0) => break, // connexion fermée
            Ok(_) => buf.push(byte[0]),
            Err(_) => return Err(HttpError::IoError),
        }
        if buf.ends_with(b"\r\n\r\n") {
            break;
        }
    }

    Ok(buf)
}

/// Sépare `/path?key=val&key2=val2` en `(path, params)`.
fn parse_path_and_params(raw: &str) -> (String, HashMap<String, String>) {
    match raw.split_once('?') {
        None => (raw.to_string(), HashMap::new()),
        Some((path, qs)) => {
            let params = qs
                .split('&')
                .filter_map(|pair| pair.split_once('='))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            (path.to_string(), params)
        }
    }
}

/// Lit `Content-Length` dans les headers (clés normalisées en minuscules).
fn content_length(headers: &HashMap<String, String>) -> Option<usize> {
    headers.get("content-length")?.trim().parse().ok()
}

fn parse_method(s: &str) -> Result<HttpMethod, HttpError> {
    match s {
        "GET" => Ok(HttpMethod::GET),
        "POST" => Ok(HttpMethod::POST),
        "PUT" => Ok(HttpMethod::PUT),
        "PATCH" => Ok(HttpMethod::PATCH),
        "HEAD" => Ok(HttpMethod::HEAD),
        "DELETE" => Ok(HttpMethod::DELETE),
        "CONNECT" => Ok(HttpMethod::CONNECT),
        "OPTIONS" => Ok(HttpMethod::OPTIONS),
        "TRACE" => Ok(HttpMethod::TRACE),
        e => Err(HttpError::MethodNotFound(e.to_string())),
    }
}

#[cfg(test)]

mod tests {

    // Helpers partagés
    use tokio::io::{AsyncWriteExt, DuplexStream};

    use crate::interface::{HttpError, HttpMethod, decode_request};

    /// Crée un faux TcpStream à partir d'une chaîne brute.
    /// On utilise tokio::io::duplex pour éviter un vrai socket.
    async fn make_stream(raw: &str) -> DuplexStream {
        let (mut server, client) = tokio::io::duplex(4096);
        server.write_all(raw.as_bytes()).await.unwrap();
        // On ferme le côté écriture pour signaler EOF au lecteur
        drop(server);
        client
    }
    // Cas nominaux
    #[tokio::test]
    async fn test_simple_get() {
        let raw = "GET /hello HTTP/1.1\r\n\
               Host: localhost\r\n\
               \r\n";
        let mut stream = make_stream(raw).await;
        let req = decode_request(&mut stream).await.unwrap();

        assert_eq!(req.method, HttpMethod::GET);
        assert_eq!(req.path, "/hello");
        assert_eq!(req.http_version, "HTTP/1.1");
        assert!(req.body.is_none());
    }

    #[tokio::test]
    async fn test_post_with_body() {
        let body = r#"{"name":"Alice"}"#;
        let raw = format!(
            "POST /users HTTP/1.1\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {}",
            body.len(),
            body,
        );
        let mut stream = make_stream(&raw).await;
        let req = decode_request(&mut stream).await.unwrap();

        assert_eq!(req.method, HttpMethod::POST);
        assert_eq!(req.body.as_deref(), Some(r#"{"name":"Alice"}"#));
    }

    #[tokio::test]
    async fn test_query_params_parsed() {
        let raw = "GET /search?q=rust&page=2 HTTP/1.1\r\n\r\n";
        let mut stream = make_stream(raw).await;
        let req = decode_request(&mut stream).await.unwrap();

        assert_eq!(req.path, "/search");
        assert_eq!(req.params.get("q").map(String::as_str), Some("rust"));
        assert_eq!(req.params.get("page").map(String::as_str), Some("2"));
    }

    #[tokio::test]
    async fn test_headers_normalized_lowercase() {
        let raw = "GET / HTTP/1.1\r\n\
               Content-Type: text/html\r\n\
               X-Custom-Header: foobar\r\n\
               \r\n";
        let mut stream = make_stream(raw).await;
        let req = decode_request(&mut stream).await.unwrap();

        // Les clés doivent être en minuscules (RFC 7230)
        assert_eq!(
            req.headers.get("content-type").map(String::as_str),
            Some("text/html")
        );
        assert_eq!(
            req.headers.get("x-custom-header").map(String::as_str),
            Some("foobar")
        );
    }

    #[tokio::test]
    async fn test_all_methods_parsed() {
        let cases = [
            ("PUT", HttpMethod::PUT),
            ("PATCH", HttpMethod::PATCH),
            ("DELETE", HttpMethod::DELETE),
            ("HEAD", HttpMethod::HEAD),
            ("OPTIONS", HttpMethod::OPTIONS),
            ("TRACE", HttpMethod::TRACE),
            ("CONNECT", HttpMethod::CONNECT),
        ];
        for (verb, expected) in cases {
            let raw = format!("{} / HTTP/1.1\r\n\r\n", verb);
            let mut stream = make_stream(&raw).await;
            let req = decode_request(&mut stream).await.unwrap();
            assert_eq!(req.method, expected, "failed for {verb}");
        }
    }
    // Erreurs attendues
    #[tokio::test]
    async fn test_unknown_method_returns_error() {
        let raw = "FLYING / HTTP/1.1\r\n\r\n";
        let mut stream = make_stream(raw).await;
        let err = decode_request(&mut stream).await.unwrap_err();

        assert_eq!(err, HttpError::MethodNotFound("FLYING".to_string()));
    }

    #[tokio::test]
    async fn test_empty_request_returns_malformed() {
        let mut stream = make_stream("\r\n\r\n").await;
        let err = decode_request(&mut stream).await.unwrap_err();

        assert_eq!(err, HttpError::MalformedRequest);
    }

    #[tokio::test]
    async fn test_invalid_utf8_returns_encoding_error() {
        // On injecte des octets invalides dans les headers
        let mut raw = b"GET / HTTP/1.1\r\nX-Broken: ".to_vec();
        raw.extend_from_slice(&[0xFF, 0xFE]); // octets UTF-8 invalides
        raw.extend_from_slice(b"\r\n\r\n");

        let (mut server, mut client) = tokio::io::duplex(4096);
        server.write_all(&raw).await.unwrap();
        drop(server);
        let err = decode_request(&mut client).await.unwrap_err();

        assert_eq!(err, HttpError::InvalidEncoding);
    }
    // Edge cases
    #[tokio::test]
    async fn test_no_query_params() {
        let raw = "GET /about HTTP/1.1\r\n\r\n";
        let mut stream = make_stream(raw).await;
        let req = decode_request(&mut stream).await.unwrap();

        assert!(req.params.is_empty());
    }

    #[tokio::test]
    async fn test_header_with_colon_in_value() {
        // "Authorization: Basic dXNlcjpwYXNz" → la valeur contient ':'
        let raw = "GET / HTTP/1.1\r\n\
               Authorization: Basic dXNlcjpwYXNz\r\n\
               \r\n";
        let mut stream = make_stream(raw).await;
        let req = decode_request(&mut stream).await.unwrap();

        // split_once(':') ne doit couper qu'au premier ':'
        assert_eq!(
            req.headers.get("authorization").map(String::as_str),
            Some("Basic dXNlcjpwYXNz"),
        );
    }

    #[tokio::test]
    async fn test_content_length_zero_gives_no_body() {
        let raw = "POST /ping HTTP/1.1\r\n\
               Content-Length: 0\r\n\
               \r\n";
        let mut stream = make_stream(raw).await;
        let req = decode_request(&mut stream).await.unwrap();

        assert!(req.body.is_none());
    }

    #[tokio::test]
    async fn test_root_path_no_params() {
        let raw = "GET / HTTP/1.1\r\n\r\n";
        let mut stream = make_stream(raw).await;
        let req = decode_request(&mut stream).await.unwrap();

        assert_eq!(req.path, "/");
        assert!(req.params.is_empty());
    }
}
