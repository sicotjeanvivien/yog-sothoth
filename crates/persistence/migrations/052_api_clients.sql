-- 052_api_clients.sql — the clients of the API and their keys. A key is stored as its SHA-256
-- alone, with a prefix an operator can name it by; only yog_migrate writes, through
-- `yog-migrate create-api-key` and `revoke-api-key`.

CREATE TABLE api_clients (
    id         BIGSERIAL   PRIMARY KEY,
    name       TEXT        NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ NULL
);

-- ⚠️ A client holds several keys, so that one is replaced without the client losing its identity.
CREATE TABLE api_keys (
    id         BIGSERIAL   PRIMARY KEY,
    client_id  BIGINT      NOT NULL REFERENCES api_clients (id),
    prefix     TEXT        NOT NULL UNIQUE CHECK (prefix ~ '^[0-9a-f]{8}$'),
    key_hash   BYTEA       NOT NULL UNIQUE CHECK (octet_length(key_hash) = 32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ NULL
);

GRANT SELECT ON api_clients, api_keys TO yog_api;
