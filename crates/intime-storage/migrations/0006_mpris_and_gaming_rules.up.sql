-- Fix MPRIS mis-categorization (browsers were media_local) and expand gaming seeds.
-- Safe to apply on existing DBs without wiping events/sessions.

-- Bare MPRIS must not outrank YouTube / Netflix title+URL rules.
UPDATE activity_rule
SET priority = 35
WHERE match_field = 'focused_control_type'
  AND match_op = 'equals'
  AND lower(pattern) = 'mpris';

INSERT OR IGNORE INTO activity_rule (category_id, match_field, match_op, pattern, priority, source, notes) VALUES
((SELECT id FROM category WHERE slug='media_streaming_official'), 'automation_id', 'contains', 'brave', 155, 'seed', 'MPRIS browser player'),
((SELECT id FROM category WHERE slug='media_streaming_official'), 'automation_id', 'contains', 'firefox', 155, 'seed', 'MPRIS browser player'),
((SELECT id FROM category WHERE slug='media_streaming_official'), 'automation_id', 'contains', 'chrome', 155, 'seed', 'MPRIS browser player'),
((SELECT id FROM category WHERE slug='media_streaming_official'), 'automation_id', 'contains', 'chromium', 155, 'seed', 'MPRIS browser player'),
((SELECT id FROM category WHERE slug='media_streaming_official'), 'automation_id', 'contains', 'edge', 155, 'seed', 'MPRIS browser player'),
((SELECT id FROM category WHERE slug='media_streaming_official'), 'automation_id', 'contains', 'vivaldi', 155, 'seed', 'MPRIS browser player'),
((SELECT id FROM category WHERE slug='media_streaming_official'), 'automation_id', 'contains', 'opera', 155, 'seed', 'MPRIS browser player'),
((SELECT id FROM category WHERE slug='music_listening'), 'automation_id', 'contains', 'spotify', 155, 'seed', 'MPRIS music player'),
((SELECT id FROM category WHERE slug='media_local'), 'automation_id', 'contains', 'vlc', 150, 'seed', 'MPRIS local player'),
((SELECT id FROM category WHERE slug='media_local'), 'automation_id', 'contains', 'mpv', 150, 'seed', 'MPRIS local player'),
((SELECT id FROM category WHERE slug='media_local'), 'automation_id', 'contains', 'jellyfin', 150, 'seed', 'MPRIS local player'),
((SELECT id FROM category WHERE slug='media_local'), 'automation_id', 'contains', 'plex', 150, 'seed', 'MPRIS local player'),
((SELECT id FROM category WHERE slug='music_listening'), 'automation_id', 'contains', 'rhythmbox', 150, 'seed', 'MPRIS music player');

-- Expanded gaming coverage (product / title / url / path / company).
INSERT OR IGNORE INTO activity_rule (category_id, match_field, match_op, pattern, priority, source) VALUES
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'league of legends', 100, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'valorant', 100, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'riot client', 100, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'osu!', 100, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'counter-strike', 100, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'dota', 100, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'path of exile', 100, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'world of warcraft', 100, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'ubisoft connect', 100, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'rockstar games', 100, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'parsec', 100, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'ppsspp', 100, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'duckstation', 100, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'window_title', 'contains', 'league of legends', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'window_title', 'contains', 'valorant', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'window_title', 'contains', 'counter-strike', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'window_title', 'contains', 'dota 2', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'window_title', 'contains', 'minecraft', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'window_title', 'contains', 'roblox', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'window_title', 'contains', 'fortnite', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'window_title', 'contains', 'path of exile', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'window_title', 'contains', 'lutris', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'window_title', 'contains', 'heroic games', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'steamdb.info', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'protondb.com', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'nexusmods.com', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'curseforge.com', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'modrinth.com', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'op.gg', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'tracker.gg', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'chess.com', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'lichess.org', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'howlongtobeat.com', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'store.epicgames.com', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'itch.io', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'executable_path', 'contains', 'steamapps', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'executable_path', 'contains', 'Heroic/Games', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'executable_path', 'contains', 'riot-client', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'executable_path', 'contains', 'Battle.net', 95, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'company', 'contains', 'riot games', 85, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'company', 'contains', 'blizzard', 85, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'company', 'contains', 'electronic arts', 85, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'company', 'contains', 'ubisoft', 85, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'company', 'contains', 'rockstar games', 85, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'company', 'contains', 'nintendo', 85, 'seed'),
((SELECT id FROM category WHERE slug='gaming'), 'aumid', 'contains', 'com.valvesoftware.Steam', 100, 'seed');
