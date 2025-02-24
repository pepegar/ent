-- Create transaction tracking table
CREATE TABLE relation_tuple_transaction (
    xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
    snapshot pg_snapshot DEFAULT pg_current_snapshot(),
    timestamp TIMESTAMP WITHOUT TIME ZONE DEFAULT (now() AT TIME ZONE 'UTC'),
    metadata JSONB NOT NULL DEFAULT '{}',
    CONSTRAINT pk_rttx PRIMARY KEY (xid)
);

-- Create schemata table
CREATE TABLE schemata (
    id BIGSERIAL PRIMARY KEY,
    type_name TEXT NOT NULL,
    schema JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

-- Create objects table
CREATE TABLE objects (
    id BIGSERIAL PRIMARY KEY,
    user_id TEXT NOT NULL,
    type TEXT NOT NULL,
    created_xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
    deleted_xid xid8 NOT NULL DEFAULT '9223372036854775807',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

-- Create metadata history table
CREATE TABLE object_metadata_history (
    id BIGSERIAL PRIMARY KEY,
    object_id BIGINT NOT NULL,
    metadata JSONB NOT NULL,
    created_xid xid8 NOT NULL,
    deleted_xid xid8 NOT NULL DEFAULT '9223372036854775807',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_object_metadata_history_object
        FOREIGN KEY (object_id)
        REFERENCES objects(id)
        ON DELETE CASCADE
);

-- Create triples table
CREATE TABLE triples (
    id BIGSERIAL PRIMARY KEY,
    user_id TEXT NOT NULL,
    from_type TEXT NOT NULL,
    from_id BIGINT NOT NULL,
    relation TEXT NOT NULL,
    to_type TEXT NOT NULL,
    to_id BIGINT NOT NULL,
    created_xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
    deleted_xid xid8 NOT NULL DEFAULT '9223372036854775807',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

-- Create edge metadata history table
CREATE TABLE edge_metadata_history (
    id BIGSERIAL PRIMARY KEY,
    edge_id BIGINT NOT NULL,
    metadata JSONB NOT NULL,
    created_xid xid8 NOT NULL,
    deleted_xid xid8 NOT NULL DEFAULT '9223372036854775807',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_edge_metadata_history_edge
        FOREIGN KEY (edge_id)
        REFERENCES triples(id)
        ON DELETE CASCADE
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_objects_type ON objects(type);
CREATE INDEX IF NOT EXISTS idx_objects_user_id ON objects(user_id);
CREATE INDEX IF NOT EXISTS idx_objects_created_xid ON objects(created_xid);
CREATE INDEX IF NOT EXISTS idx_objects_deleted_xid ON objects(deleted_xid);

CREATE INDEX IF NOT EXISTS idx_object_metadata_history_object_id ON object_metadata_history(object_id);
CREATE INDEX IF NOT EXISTS idx_object_metadata_history_created_xid ON object_metadata_history(created_xid);
CREATE INDEX IF NOT EXISTS idx_object_metadata_history_deleted_xid ON object_metadata_history(deleted_xid);

CREATE INDEX IF NOT EXISTS idx_triples_from ON triples(from_type, from_id);
CREATE INDEX IF NOT EXISTS idx_triples_to ON triples(to_type, to_id);
CREATE INDEX IF NOT EXISTS idx_triples_user_id ON triples(user_id);
CREATE INDEX IF NOT EXISTS idx_triples_relation ON triples(relation);
CREATE INDEX IF NOT EXISTS idx_triples_created_xid ON triples(created_xid);
CREATE INDEX IF NOT EXISTS idx_triples_deleted_xid ON triples(deleted_xid);

CREATE INDEX IF NOT EXISTS idx_edge_metadata_history_edge_id ON edge_metadata_history(edge_id);
CREATE INDEX IF NOT EXISTS idx_edge_metadata_history_created_xid ON edge_metadata_history(created_xid);
CREATE INDEX IF NOT EXISTS idx_edge_metadata_history_deleted_xid ON edge_metadata_history(deleted_xid);

CREATE INDEX IF NOT EXISTS idx_transaction_timestamp ON relation_tuple_transaction(timestamp);

-- Add foreign key constraints
ALTER TABLE triples
    ADD CONSTRAINT fk_triples_from
    FOREIGN KEY (from_id)
    REFERENCES objects(id)
    ON DELETE CASCADE;

ALTER TABLE triples
    ADD CONSTRAINT fk_triples_to
    FOREIGN KEY (to_id)
    REFERENCES objects(id)
    ON DELETE CASCADE;
