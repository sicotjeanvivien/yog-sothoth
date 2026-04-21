use std::fmt;

/// URL potentiellement porteuse d'un secret (ex: `?api-key=...`).
///
/// Les impls `Display` et `Debug` **masquent** les paramètres de query
/// pour éviter toute fuite via les logs ou les erreurs. Pour obtenir
/// l'URL brute (à passer au client RPC), appeler [`expose`].
#[derive(Clone)]
pub(crate) struct SecretUrl(String);

impl SecretUrl {
    pub(crate) fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// Retourne l'URL brute. À n'appeler qu'au moment de la consommer
    /// (construction d'un client HTTP, d'un WebSocket, etc.).
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SecretUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", redact(&self.0))
    }
}

impl fmt::Debug for SecretUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Même traitement en Debug — indispensable car `{:?}` est
        // souvent utilisé dans les macros tracing et les erreurs.
        write!(f, "SecretUrl({})", redact(&self.0))
    }
}

/// Remplace la portion `?query_string` par `?***REDACTED***`.
///
/// Volontairement simple : on n'essaie pas de préserver les paramètres
/// non-sensibles. Tout ce qui suit `?` est masqué. Si un jour on a
/// besoin de conserver certains params, on fera un vrai parsing d'URL.
fn redact(url: &str) -> String {
    match url.find('?') {
        Some(idx) => format!("{}?***REDACTED***", &url[..idx]),
        None => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_redacts_query_string() {
        let url = SecretUrl::new("https://mainnet.helius-rpc.com/?api-key=abc123");
        assert_eq!(
            format!("{url}"),
            "https://mainnet.helius-rpc.com/?***REDACTED***"
        );
    }

    #[test]
    fn debug_redacts_query_string() {
        let url = SecretUrl::new("wss://mainnet.helius-rpc.com/?api-key=abc123");
        assert_eq!(
            format!("{url:?}"),
            "SecretUrl(wss://mainnet.helius-rpc.com/?***REDACTED***)"
        );
    }

    #[test]
    fn expose_returns_raw_url() {
        let raw = "https://mainnet.helius-rpc.com/?api-key=abc123";
        let url = SecretUrl::new(raw);
        assert_eq!(url.expose(), raw);
    }

    #[test]
    fn url_without_query_is_unchanged_in_display() {
        let url = SecretUrl::new("https://api.mainnet-beta.solana.com");
        assert_eq!(format!("{url}"), "https://api.mainnet-beta.solana.com");
    }
}