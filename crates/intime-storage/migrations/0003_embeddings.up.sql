-- SELECT load_extension('target/debug/sqlite_vec0.dll');
CREATE TABLE
    IF NOT EXISTS embedding (
        id INTEGER PRIMARY KEY autoincrement NOT NULL,
        event_id INTEGER,
        embedding_type TEXT NOT NULL,
        embedding_backend TEXT NOT NULL,
        FOREIGN KEY (event_id) REFERENCES event (id)
    );

CREATE VIRTUAL TABLE IF NOT EXISTS vec_embedding USING vec0(
    id INTEGER PRIMARY KEY,
    -- This might not be enough for different backends. Vit B 32 is 512
    embedding_data float[512]
);