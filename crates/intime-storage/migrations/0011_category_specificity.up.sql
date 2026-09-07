-- Narrow vague "Browsing", rename consumer labels, add web_search + gaming guide rules.
-- Safe to apply on existing DBs without wiping events/sessions.

INSERT OR IGNORE INTO category (slug, name, description, occupation_tags, source)
VALUES (
  'web_search',
  'Web search',
  'Search engines and query result pages',
  '["general"]',
  'seed'
);

UPDATE category SET name = 'Other web browsing',
    description = 'Generic browser fallback when no site-specific category matches'
WHERE slug = 'browsing';

UPDATE category SET name = 'Watching series / streaming',
    description = 'Licensed TV, movies, and video streaming (Netflix, YouTube watch, Prime, …)'
WHERE slug = 'media_streaming_official';

UPDATE category SET name = 'Unofficial streaming',
    description = 'Free/unofficial film and TV sites'
WHERE slug = 'media_streaming_unofficial';

UPDATE category SET name = 'Social media',
    description = 'X/Twitter, Instagram, Reddit, LinkedIn, YouTube home/channels, forums'
WHERE slug = 'social_long';

UPDATE category SET name = 'Short-form video',
    description = 'TikTok, Reels, Shorts, Stories'
WHERE slug = 'social_short';

-- Web search (must beat browsing @55).
INSERT OR IGNORE INTO activity_rule (category_id, match_field, match_op, pattern, priority, source, notes) VALUES
((SELECT id FROM category WHERE slug='web_search'), 'url', 'contains', 'google.com/search', 125, 'seed', 'Google Search'),
((SELECT id FROM category WHERE slug='web_search'), 'url', 'contains', 'www.google.com/search', 125, 'seed', 'Google Search'),
((SELECT id FROM category WHERE slug='web_search'), 'url', 'contains', 'bing.com/search', 125, 'seed', 'Bing'),
((SELECT id FROM category WHERE slug='web_search'), 'url', 'contains', 'duckduckgo.com', 125, 'seed', 'DuckDuckGo'),
((SELECT id FROM category WHERE slug='web_search'), 'url', 'contains', 'search.brave.com', 125, 'seed', 'Brave Search'),
((SELECT id FROM category WHERE slug='web_search'), 'url', 'contains', 'search.yahoo.com', 120, 'seed', 'Yahoo Search'),
((SELECT id FROM category WHERE slug='web_search'), 'window_title', 'contains', ' - google search', 120, 'seed', 'Google Search title'),
((SELECT id FROM category WHERE slug='web_search'), 'window_title', 'contains', ' - bing', 115, 'seed', 'Bing title'),
((SELECT id FROM category WHERE slug='web_search'), 'window_title', 'contains', ' - duckduckgo', 115, 'seed', 'DDG title'),
((SELECT id FROM category WHERE slug='web_search'), 'window_title', 'contains', 'google search', 118, 'seed', 'Google Search title'),
((SELECT id FROM category WHERE slug='web_search'), 'url', 'contains', 'google.com/webhp', 110, 'seed', 'Google homepage');

-- Gaming guides / trackers that otherwise fall into browsing.
INSERT OR IGNORE INTO activity_rule (category_id, match_field, match_op, pattern, priority, source, notes) VALUES
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'tftflow.com', 120, 'seed', 'TFT Flow'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'tftacademy.com', 120, 'seed', 'TFT Academy'),
((SELECT id FROM category WHERE slug='gaming'), 'window_title', 'contains', 'tft flow', 115, 'seed', 'TFT Flow title'),
((SELECT id FROM category WHERE slug='gaming'), 'window_title', 'contains', 'tft academy', 115, 'seed', 'TFT Academy title'),
((SELECT id FROM category WHERE slug='gaming'), 'window_title', 'contains', ' | tft flow', 115, 'seed', 'TFT Flow title'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'op.gg', 115, 'seed', 'op.gg'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'u.gg', 115, 'seed', 'u.gg'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'mobafire.com', 110, 'seed', 'Mobafire'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'lolalytics.com', 110, 'seed', 'Lolalytics'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'maxroll.gg', 110, 'seed', 'Maxroll'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'wowhead.com', 110, 'seed', 'Wowhead'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'poe.ninja', 110, 'seed', 'poe.ninja'),
((SELECT id FROM category WHERE slug='gaming'), 'window_title', 'contains', 'op.gg', 110, 'seed', 'op.gg title');

-- Strengthen Netflix / YouTube watch titles so they beat weak browsing.
INSERT OR IGNORE INTO activity_rule (category_id, match_field, match_op, pattern, priority, source, notes) VALUES
((SELECT id FROM category WHERE slug='media_streaming_official'), 'window_title', 'contains', ' - netflix', 135, 'seed', 'Netflix tab title'),
((SELECT id FROM category WHERE slug='media_streaming_official'), 'window_title', 'contains', 'netflix - ', 135, 'seed', 'Netflix tab title'),
((SELECT id FROM category WHERE slug='media_streaming_official'), 'url', 'contains', 'netflix.com/watch', 140, 'seed', 'Netflix watch URL'),
((SELECT id FROM category WHERE slug='media_streaming_official'), 'url', 'contains', 'netflix.com/title', 138, 'seed', 'Netflix title URL');
