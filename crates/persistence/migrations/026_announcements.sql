-- ============================================================================
-- 026 — announcements
-- ============================================================================
-- Operator → users one-way communication channel (v0.1.1 release prep).
-- An announcement (maintenance, incident, release, beta) must be publishable
-- WITHOUT a deploy — hence a table served by the api, not static web content.
-- The changelog page is the static counterpart; a 'release' announcement
-- points at it via link_url.
--
-- Deliberately NOT a hypertable: an operator-curated table of a handful of
-- rows with no time-series semantics — it joins the generic single-table
-- family (network_status), not the event family.
--
-- Severity deliberately does NOT reuse the Signal Engine scale: a signal
-- severity is a detector's *business* conclusion (escalation semantics,
-- dedup); an announcement severity is the operator's *editorial* display
-- choice. Three same-named tags today is vocabulary coincidence, not concept
-- identity — each side keeps its own enum and CHECK.
--
-- Publication is an operator INSERT/UPDATE via psql (admin); the
-- authenticated write endpoint is deferred to auth (v0.3). No runtime role
-- gets write access — yog_api stays read-only by design.
-- ============================================================================

CREATE TABLE announcements (
    id         BIGSERIAL PRIMARY KEY,
    kind       TEXT        NOT NULL,   -- what it is about (label chip on the web)
    severity   TEXT        NOT NULL,   -- how prominently to display it
    message    TEXT        NOT NULL,   -- free operator text (English, v1 decision)
    link_url   TEXT,                   -- optional target, e.g. /changelog#v0.1.1
    starts_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ends_at    TIMESTAMPTZ,            -- NULL = shown until the operator closes it
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT announcements_kind_valid
        CHECK (kind IN ('maintenance', 'incident', 'release', 'beta')),
    CONSTRAINT announcements_severity_valid
        CHECK (severity IN ('info', 'warning', 'critical')),
    CONSTRAINT announcements_window_valid
        CHECK (ends_at IS NULL OR ends_at > starts_at)
);

-- The active-window read scans a handful of rows — no index beyond the PK.

GRANT SELECT ON announcements TO yog_api;
