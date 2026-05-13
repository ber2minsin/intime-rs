CREATE TABLE
    IF NOT EXISTS app (
        id INTEGER PRIMARY KEY autoincrement NOT NULL,
        fingerprint BLOB NOT NULL UNIQUE, -- Generated on runtime
        fingerprint_version INTEGER NOT NULL DEFAULT 1,
        display_name TEXT NOT NULL,
        product_name TEXT,
        product_version TEXT,
        file_path TEXT NOT NULL,
        company_id INTEGER,
        first_seen_at TIMESTAMP,
        created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (company_id) REFERENCES company (id)
    );

CREATE TABLE
    IF NOT EXISTS company (
        id INTEGER PRIMARY KEY autoincrement NOT NULL,
        company_name TEXT NOT NULL,
        created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
    );

CREATE INDEX IF NOT EXISTS app_fingerprint_idx ON app (fingerprint);