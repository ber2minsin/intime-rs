-- Screenshot lifecycle + pin-worthy sessions.
-- important sessions keep raw screenshots (retention worker skips them).
-- screenshot_tier tracks degradation: full → compact → (path cleared).

ALTER TABLE session ADD COLUMN important INTEGER NOT NULL DEFAULT 0;

ALTER TABLE event ADD COLUMN screenshot_tier TEXT;
