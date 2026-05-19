-- 004_token_enrichment.sql
--
-- Token enrichment tables, populated by the `yog-context` daemon:
--   - token_metadata : symbol / name / decimals / logo, from Helius DAS
--   - token_prices   : USD price time series, from Jupiter
--
-- These enrich the raw on-chain data: the indexer records mints as
-- bare addresses, `yog-context` gives them an identity and a price.

-- ---------------------------------------------------------------------------
-- token_metadata — one row per mint, near-immutable reference data.
--
-- NOT a hypertable: this is slow-changing reference data, one row per
-- mint, refreshed rarely. A plain relational table is the right fit.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS token_metadata (
    -- The SPL mint address (base58). Primary key — one row per mint.
    mint              TEXT        PRIMARY KEY,

    -- Symbol and name. NULLABLE on purpose: some tokens (very old, or
    -- raw launches) carry no Metaplex metadata. DAS still returns
    -- decimals in that case, so the row is kept with name/symbol null.
    symbol            TEXT,
    name              TEXT,

    -- Token decimal precision. NOT NULL — DAS always provides it, and
    -- it is the field the indexer most needs to render raw amounts.
    decimals          SMALLINT    NOT NULL,

    -- Logo URI, as returned by DAS. May be an ipfs:// URI — stored
    -- verbatim, the frontend resolves it.
    logo_uri          TEXT,

    -- Which source produced this row. A single value for now
    -- ('helius_das'), kept explicit for future-proofing and debug.
    metadata_source   TEXT        NOT NULL DEFAULT 'helius_das',

    -- When the row was first fetched, and when it was last refreshed.
    fetched_at        TIMESTAMPTZ NOT NULL,
    last_refresh_at   TIMESTAMPTZ NOT NULL
);

-- Supports "least recently refreshed" scans, if a refresh policy is
-- added later.
CREATE INDEX IF NOT EXISTS idx_token_metadata_last_refresh
    ON token_metadata (last_refresh_at);

-- ---------------------------------------------------------------------------
-- token_prices — USD price time series, one row per (mint, fetch).
--
-- A hypertable: this is pure time-series data. Compression and
-- retention policies are intentionally NOT set up here — at the v0.1
-- scale (a handful of watched pools) the volume is tiny. Policies
-- will be added when the watched-pool allowlist is lifted and the
-- row count actually justifies them.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS token_prices (
    -- The SPL mint this price is for. No FK to token_metadata: a
    -- price may, in principle, be fetched before metadata exists.
    mint          TEXT            NOT NULL,

    -- Price in USD. NUMERIC(38, 18) covers memecoins with very small
    -- per-token values without precision loss.
    price_usd     NUMERIC(38, 18) NOT NULL,

    -- Which source produced this price: 'jupiter' | 'helius' |
    -- 'fallback'. Explicit, to avoid confusion when debugging.
    price_source  TEXT            NOT NULL,

    -- Optional confidence value (Jupiter sometimes provides one).
    confidence    REAL,

    -- When the price was fetched. Part of the PK and the hypertable
    -- time dimension.
    fetched_at    TIMESTAMPTZ     NOT NULL,

    PRIMARY KEY (mint, fetched_at)
);

-- Promote to a hypertable on the fetch-time dimension.
SELECT create_hypertable(
    'token_prices',
    'fetched_at',
    chunk_time_interval => INTERVAL '7 days',
    if_not_exists       => TRUE
);

-- Supports "latest price for a mint" lookups (mint filter + recent
-- ordering).
CREATE INDEX IF NOT EXISTS idx_token_prices_mint_recent
    ON token_prices (mint, fetched_at DESC);