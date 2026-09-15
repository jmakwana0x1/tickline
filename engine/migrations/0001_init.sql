-- Phase 0: the smallest schema that proves the migration harness runs.
--
-- The real ledger tables land in Phase 4. What is set up here is the one rule the whole
-- ledger depends on and that is easiest to enforce from the very first migration: entries
-- are append-only (I8). A table added later inherits the guard by calling this trigger.

CREATE TABLE schema_meta (
    key         text PRIMARY KEY,
    value       text        NOT NULL,
    updated_at  timestamptz NOT NULL DEFAULT now()
);

INSERT INTO schema_meta (key, value) VALUES ('phase', '0');

-- Attach to any append-only table:  CREATE TRIGGER ... EXECUTE FUNCTION forbid_mutation();
CREATE FUNCTION forbid_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION
        'table % is append-only (CLAUDE.md I8): % is not permitted',
        TG_TABLE_NAME, TG_OP;
END;
$$;
