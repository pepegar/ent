-- Create transaction tracking table
CREATE TABLE relation_tuple_transaction (
    xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
    snapshot pg_snapshot DEFAULT pg_current_snapshot(),
    timestamp TIMESTAMP WITHOUT TIME ZONE DEFAULT (now() AT TIME ZONE 'UTC'),
    metadata JSONB NOT NULL DEFAULT '{}',
    CONSTRAINT pk_rttx PRIMARY KEY (xid)
);

-- Add transaction tracking to objects
ALTER TABLE objects
    ADD COLUMN created_xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
    ADD COLUMN deleted_xid xid8 NOT NULL DEFAULT '9223372036854775807';

-- Add transaction tracking to triples
ALTER TABLE triples  
    ADD COLUMN created_xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
    ADD COLUMN deleted_xid xid8 NOT NULL DEFAULT '9223372036854775807';

-- Add indexes for performance
CREATE INDEX idx_objects_created_xid ON objects(created_xid);
CREATE INDEX idx_objects_deleted_xid ON objects(deleted_xid);
CREATE INDEX idx_triples_created_xid ON triples(created_xid);
CREATE INDEX idx_triples_deleted_xid ON triples(deleted_xid);
CREATE INDEX idx_transaction_timestamp ON relation_tuple_transaction(timestamp);
