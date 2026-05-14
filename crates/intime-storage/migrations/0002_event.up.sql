CREATE TABLE
    IF NOT EXISTS event (
        id INTEGER PRIMARY KEY autoincrement NOT NULL,
        app_id INTEGER,
        event_type TEXT NOT NULL,
        occured_at TIMESTAMP NOT NULL,
        created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
        payload TEXT,
        screenshot_path TEXT,
        -- AI stuff
        FOREIGN KEY (app_id) REFERENCES app (id)
    );