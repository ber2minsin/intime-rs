-- URL is site-specific: keep it in event.payload JSON metadata, not a column.
-- Also fix social vs browsing classification and over-eager Brave→streaming MPRIS rules.

ALTER TABLE event DROP COLUMN url;

UPDATE category
SET name = 'Long-form social',
    description = 'X/Twitter, Instagram, LinkedIn, Reddit, blogs, forums'
WHERE slug = 'social_long';

-- Browser MPRIS must not force streaming (X/IG tabs also expose MPRIS).
DELETE FROM activity_rule
WHERE match_field = 'automation_id'
  AND match_op = 'contains'
  AND lower(pattern) IN ('brave', 'firefox', 'chrome', 'chromium', 'edge', 'vivaldi', 'opera')
  AND category_id = (SELECT id FROM category WHERE slug = 'media_streaming_official');

-- High-priority title matchers so X/Instagram beat generic browsing.
INSERT OR IGNORE INTO activity_rule (category_id, match_field, match_op, pattern, priority, source, notes) VALUES
((SELECT id FROM category WHERE slug='social_long'), 'window_title', 'contains', ' / x', 140, 'seed', 'X/Twitter tab titles'),
((SELECT id FROM category WHERE slug='social_long'), 'window_title', 'contains', ' on x:', 140, 'seed', 'X/Twitter post titles'),
((SELECT id FROM category WHERE slug='social_long'), 'window_title', 'contains', 'instagram', 140, 'seed', 'Instagram tabs'),
((SELECT id FROM category WHERE slug='social_long'), 'window_title', 'contains', 'x.com', 140, 'seed', 'X URL in title'),
((SELECT id FROM category WHERE slug='social_long'), 'window_title', 'contains', 'twitter.com', 140, 'seed', 'Twitter URL in title'),
((SELECT id FROM category WHERE slug='social_short'), 'window_title', 'contains', 'instagram reels', 145, 'seed', 'IG Reels'),
((SELECT id FROM category WHERE slug='social_short'), 'url', 'contains', 'instagram.com/reel', 145, 'seed', 'IG Reels URL'),
((SELECT id FROM category WHERE slug='social_long'), 'url', 'contains', 'x.com', 140, 'seed', 'X URL metadata'),
((SELECT id FROM category WHERE slug='social_long'), 'url', 'contains', 'twitter.com', 140, 'seed', 'Twitter URL metadata'),
((SELECT id FROM category WHERE slug='social_long'), 'url', 'contains', 'instagram.com', 135, 'seed', 'Instagram URL metadata');
