CREATE TABLE packages (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    downloads BIGINT NOT NULL CHECK (downloads >= 0)
);
