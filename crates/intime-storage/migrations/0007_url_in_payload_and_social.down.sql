-- Best-effort reverse of 0007. Dropped url column is not restored on SQLite.

DELETE FROM activity_rule
WHERE source = 'seed'
  AND notes IN (
    'X/Twitter tab titles',
    'X/Twitter post titles',
    'Instagram tabs',
    'X URL in title',
    'Twitter URL in title',
    'IG Reels',
    'IG Reels URL',
    'X URL metadata',
    'Twitter URL metadata',
    'Instagram URL metadata'
  );

UPDATE category
SET name = 'Long-form social',
    description = 'YouTube, LinkedIn, blogs, forums'
WHERE slug = 'social_long';
