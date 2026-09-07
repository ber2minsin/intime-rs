-- Down migration is best-effort; SQLite cannot cheaply drop columns.
DROP TABLE IF EXISTS setting;
DROP INDEX IF EXISTS action_session_idx;
DROP INDEX IF EXISTS event_occured_at_idx;
DROP INDEX IF EXISTS event_document_idx;
DROP INDEX IF EXISTS event_action_idx;
DROP INDEX IF EXISTS event_session_idx;
DROP TABLE IF EXISTS action;
DROP TABLE IF EXISTS session;
DROP TABLE IF EXISTS app_signature;
DROP TABLE IF EXISTS app_version_info;
DROP INDEX IF EXISTS company_name_uidx;
DROP TABLE IF EXISTS aumid;
