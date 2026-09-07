-- Reverse 0005: recreate empty action table; drop rules/categories.
-- Note: dropped event.action_id is not restored (SQLite).

DROP INDEX IF EXISTS session_context_idx;
DROP INDEX IF EXISTS session_category_idx;
DROP INDEX IF EXISTS activity_rule_priority_idx;
DROP INDEX IF EXISTS activity_rule_category_idx;

DROP TABLE IF EXISTS activity_rule;
DROP TABLE IF EXISTS category;

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
