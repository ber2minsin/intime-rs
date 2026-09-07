-- Focused UI fields are sparse / toolkit-specific: keep them in payload JSON only.

ALTER TABLE event DROP COLUMN focused_element;
ALTER TABLE event DROP COLUMN focused_element_class;
ALTER TABLE event DROP COLUMN focused_control_type;
ALTER TABLE event DROP COLUMN automation_id;
