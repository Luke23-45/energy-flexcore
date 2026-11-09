-- P1.13 -- Migration 0007 up: outbox (P1.12 §4.6; report §6.3, §15.5).
--
-- Transactional outbox: written in the SAME PostgreSQL transaction as the
-- state change it belongs to. Infrastructure working queue, NOT
-- tenant-owned: no RLS (P1.12 §5.2); the payload carries tenant attribution
-- and access is flexcore_app-only. The event's event_time lives in
-- payload.event_time (P1.09); created_at/updated_at are delivery-lifecycle
-- times so the P0.09 distinction is never silently lost (P1.12 §2.2).

CREATE TABLE outbox (
    outbox_id           uuid        PRIMARY KEY DEFAULT uuidv7(),
    idempotency_key     text        NOT NULL UNIQUE,
    event_type          text        NOT NULL,
    schema_version      text        NOT NULL,
    payload             jsonb       NOT NULL,
    destination_subject text        NOT NULL CHECK (length(btrim(destination_subject)) > 0),
    delivery_state      text        NOT NULL
                        CHECK (delivery_state IN ('pending', 'delivered', 'failed')),
    attempt_count       integer     NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    created_at          timestamptz NOT NULL DEFAULT now(),
    updated_at          timestamptz NOT NULL DEFAULT now(),
    delivered_at        timestamptz NULL  -- set only after broker acknowledgement (§6.3 step 3)
);

-- Pending scan for the outbox publisher (P1.12 §4.6; P1.15).
CREATE INDEX outbox_pending_idx ON outbox (created_at) WHERE delivery_state = 'pending';
-- Monitoring / backlog queries.
CREATE INDEX outbox_state_idx ON outbox (delivery_state, created_at);

ALTER TABLE outbox OWNER TO flexcore_migrator;

REVOKE ALL ON outbox FROM PUBLIC;
GRANT SELECT, INSERT, UPDATE ON outbox TO flexcore_app;
