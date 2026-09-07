-- Revert 0011 category specificity changes (best-effort; seed rows may remain if ids shifted).

DELETE FROM activity_rule
WHERE notes IN (
  'Google Search', 'Bing', 'DuckDuckGo', 'Brave Search', 'Yahoo Search',
  'Google Search title', 'Bing title', 'DDG title', 'Google homepage',
  'TFT Flow', 'TFT Academy', 'TFT Flow title', 'TFT Academy title',
  'op.gg', 'u.gg', 'Mobafire', 'Lolalytics', 'Maxroll', 'Wowhead', 'poe.ninja', 'op.gg title',
  'Netflix tab title', 'Netflix watch URL', 'Netflix title URL'
)
AND source = 'seed';

DELETE FROM category WHERE slug = 'web_search';

UPDATE category SET name = 'Browsing',
    description = 'Generic web browsing (fallback)'
WHERE slug = 'browsing';

UPDATE category SET name = 'Official streaming',
    description = 'Licensed TV/movie/music streaming'
WHERE slug = 'media_streaming_official';

UPDATE category SET name = 'Unofficial streaming',
    description = 'Free/unofficial film & TV sites'
WHERE slug = 'media_streaming_unofficial';

UPDATE category SET name = 'Long-form social',
    description = 'X/Twitter, Instagram, LinkedIn, Reddit, blogs, forums'
WHERE slug = 'social_long';

UPDATE category SET name = 'Short-form social',
    description = 'TikTok, Reels, Shorts, Stories'
WHERE slug = 'social_short';
