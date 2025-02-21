-- First add the column without constraints
ALTER TABLE schemata ADD COLUMN type_name TEXT;

-- Update existing rows with unique values
UPDATE schemata SET type_name = 'type_' || id::text WHERE type_name IS NULL;

-- Now make it NOT NULL
ALTER TABLE schemata ALTER COLUMN type_name SET NOT NULL;

-- Finally add the unique constraint
ALTER TABLE schemata ADD CONSTRAINT schemata_type_name_unique UNIQUE (type_name); 