-- Browser MPRIS players stream web media (YouTube/Netflix), not local files.
-- Without this, play_media with a bare track title matches media_local (mpris=35)
-- and splits from the Brave tab session (media_streaming_official).
INSERT INTO activity_rule(category_id, match_field, match_op, pattern, priority, enabled, source, notes)
VALUES
((SELECT id FROM category WHERE slug='media_streaming_official'), 'automation_id', 'contains', 'brave', 148, 1, 'seed', 'Browser MPRIS → streaming'),
((SELECT id FROM category WHERE slug='media_streaming_official'), 'automation_id', 'contains', 'chrome', 148, 1, 'seed', 'Browser MPRIS → streaming'),
((SELECT id FROM category WHERE slug='media_streaming_official'), 'automation_id', 'contains', 'chromium', 148, 1, 'seed', 'Browser MPRIS → streaming'),
((SELECT id FROM category WHERE slug='media_streaming_official'), 'automation_id', 'contains', 'firefox', 148, 1, 'seed', 'Browser MPRIS → streaming'),
((SELECT id FROM category WHERE slug='media_streaming_official'), 'automation_id', 'contains', 'msedge', 148, 1, 'seed', 'Browser MPRIS → streaming'),
((SELECT id FROM category WHERE slug='media_streaming_official'), 'automation_id', 'contains', 'edge', 147, 1, 'seed', 'Browser MPRIS → streaming'),
((SELECT id FROM category WHERE slug='media_streaming_official'), 'automation_id', 'contains', 'vivaldi', 147, 1, 'seed', 'Browser MPRIS → streaming'),
((SELECT id FROM category WHERE slug='media_streaming_official'), 'automation_id', 'contains', 'opera', 147, 1, 'seed', 'Browser MPRIS → streaming'),
-- nvim hosted in foot/kitty still looks like a terminal app; title wins.
((SELECT id FROM category WHERE slug='code_editing'), 'window_title', 'contains', 'nvim ', 112, 1, 'seed', 'Neovim in terminal → code_editing'),
((SELECT id FROM category WHERE slug='code_editing'), 'window_title', 'contains', 'nvim', 111, 1, 'seed', 'Neovim title'),
((SELECT id FROM category WHERE slug='code_editing'), 'window_title', 'contains', 'neovim', 111, 1, 'seed', 'Neovim title')
ON CONFLICT(match_field, match_op, pattern) DO UPDATE SET
  category_id = excluded.category_id,
  priority = excluded.priority,
  enabled = 1,
  source = excluded.source,
  notes = excluded.notes;
