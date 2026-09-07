-- Reverse of 0008: restore focused UI columns (values not recovered from payload).

ALTER TABLE event ADD COLUMN focused_element TEXT;
ALTER TABLE event ADD COLUMN focused_element_class TEXT;
ALTER TABLE event ADD COLUMN focused_control_type TEXT;
ALTER TABLE event ADD COLUMN automation_id TEXT;
