CREATE TABLE packages (
    id UUID PRIMARY KEY,
    registry TEXT NOT NULL,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    downloads BIGINT NOT NULL CHECK (downloads >= 0),
    UNIQUE (registry, name)
);
