-- Normalize app identity (aumid / version / signature) and expand event rows
-- with queryable context. Introduce sessions/actions for higher-level timeline.

CREATE TABLE IF NOT EXISTS aumid (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    aumid TEXT NOT NULL UNIQUE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX IF NOT EXISTS company_name_uidx ON company (company_name);

CREATE TABLE IF NOT EXISTS app_version_info (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    app_id INTEGER NOT NULL,
    file_description TEXT,
    product_version TEXT,
    file_version TEXT,
    original_filename TEXT,
    internal_name TEXT,
    legal_copyright TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (app_id) REFERENCES app (id)
);

CREATE TABLE IF NOT EXISTS app_signature (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    app_id INTEGER NOT NULL,
    publisher TEXT,
    subject_full TEXT,
    issuer TEXT,
    serial_number TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (app_id) REFERENCES app (id)
);

-- SQLite: add columns to existing app / event tables
ALTER TABLE app ADD COLUMN aumid_id INTEGER REFERENCES aumid (id);

CREATE TABLE IF NOT EXISTS session (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    started_at TIMESTAMP NOT NULL,
    ended_at TIMESTAMP,
    intent TEXT,
    intent_confidence REAL,
    title TEXT,
    summary TEXT,
    source TEXT NOT NULL DEFAULT 'heuristic',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS action (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    session_id INTEGER,
    started_at TIMESTAMP NOT NULL,
    ended_at TIMESTAMP,
    action_type TEXT,
    label TEXT,
    app_id INTEGER,
    document_path TEXT,
    metadata_json TEXT,
    source TEXT NOT NULL DEFAULT 'heuristic',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (session_id) REFERENCES session (id),
    FOREIGN KEY (app_id) REFERENCES app (id)
);

ALTER TABLE event ADD COLUMN window_handle INTEGER;
ALTER TABLE event ADD COLUMN window_title TEXT;
ALTER TABLE event ADD COLUMN process_id INTEGER;
ALTER TABLE event ADD COLUMN executable_path TEXT;
ALTER TABLE event ADD COLUMN focused_element TEXT;
ALTER TABLE event ADD COLUMN focused_element_class TEXT;
ALTER TABLE event ADD COLUMN focused_control_type TEXT;
ALTER TABLE event ADD COLUMN automation_id TEXT;
ALTER TABLE event ADD COLUMN document_path TEXT;
ALTER TABLE event ADD COLUMN document_name TEXT;
ALTER TABLE event ADD COLUMN url TEXT;
ALTER TABLE event ADD COLUMN workspace_path TEXT;
ALTER TABLE event ADD COLUMN text_changed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE event ADD COLUMN session_id INTEGER REFERENCES session (id);
ALTER TABLE event ADD COLUMN action_id INTEGER REFERENCES action (id);

CREATE INDEX IF NOT EXISTS event_session_idx ON event (session_id);
CREATE INDEX IF NOT EXISTS event_action_idx ON event (action_id);
CREATE INDEX IF NOT EXISTS event_document_idx ON event (document_path);
CREATE INDEX IF NOT EXISTS event_occured_at_idx ON event (occured_at);
CREATE INDEX IF NOT EXISTS action_session_idx ON action (session_id);

CREATE TABLE IF NOT EXISTS setting (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
