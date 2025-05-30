CREATE TABLE jobs (
    id UUID PRIMARY KEY,
    registry TEXT NOT NULL,
    package_name TEXT NOT NULL,
    status TEXT NOT NULL,
    trace_id TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    CHECK (status IN ('processing', 'completed'))
);
