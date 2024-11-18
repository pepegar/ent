-- Create schema version table
CREATE TABLE IF NOT EXISTS _schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

-- Create schemata table
CREATE TABLE IF NOT EXISTS schemata (
    id SERIAL PRIMARY KEY,
    schema JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

-- Create objects table
CREATE TABLE IF NOT EXISTS objects (
    id SERIAL PRIMARY KEY,
    type TEXT NOT NULL,
    metadata JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

-- Create triples table
CREATE TABLE IF NOT EXISTS triples (
    id SERIAL PRIMARY KEY,
    from_type TEXT NOT NULL,
    from_id INTEGER NOT NULL,
    relation TEXT NOT NULL,
    to_type TEXT NOT NULL,
    to_id INTEGER NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes
CREATE INDEX idx_objects_type ON objects(type);
CREATE INDEX idx_triples_from ON triples(from_type, from_id);
CREATE INDEX idx_triples_to ON triples(to_type, to_id);
CREATE INDEX idx_triples_relation ON triples(relation);

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