-- Create schemata table
CREATE TABLE IF NOT EXISTS schemata (
    id BIGSERIAL PRIMARY KEY,
    schema JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

-- Create objects table
CREATE TABLE IF NOT EXISTS objects (
    id BIGSERIAL PRIMARY KEY,
    user_id TEXT NOT NULL,
    type TEXT NOT NULL,
    metadata JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

-- Create triples table
CREATE TABLE IF NOT EXISTS triples (
    id BIGSERIAL PRIMARY KEY,
    user_id TEXT NOT NULL,
    from_type TEXT NOT NULL,
    from_id BIGINT NOT NULL,
    relation TEXT NOT NULL,
    to_type TEXT NOT NULL,
    to_id BIGINT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes
CREATE INDEX idx_objects_type ON objects(type);
CREATE INDEX idx_objects_user_id ON objects(user_id);
CREATE INDEX idx_triples_from ON triples(from_type, from_id);
CREATE INDEX idx_triples_to ON triples(to_type, to_id);
CREATE INDEX idx_triples_user_id ON triples(user_id);
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
