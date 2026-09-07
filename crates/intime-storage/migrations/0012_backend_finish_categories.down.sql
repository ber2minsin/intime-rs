DELETE FROM activity_rule
WHERE source = 'seed'
  AND notes IN (
    'GitHub code/PR', 'GitHub in title', 'PR title', 'GitHub tab chrome', 'GitLab', 'Bitbucket', 'Codeberg',
    'GitHub issues list', 'GitHub projects', 'GitHub Actions',
    'docs.rs', 'Rust docs', 'MDN', 'Stack Overflow', 'Stack Exchange', 'SO title', 'MS Learn', 'Go packages',
    'Steam', 'Epic', 'Riot Client', 'Battle.net', 'Battle.net Agent', 'EA App', 'Ubisoft', 'GOG', 'Heroic',
    'Lutris', 'Legendary', 'Bottles', 'ProtonUp', 'Steam aumid', 'Riot aumid', 'Steam apps path', 'Epic path',
    'Riot path', 'Battle.net path', 'Steam title', 'Epic title', 'Steam store', 'Epic store',
    'GnuCash', 'Quicken', 'Wave', 'Zoho Books', 'FreshBooks'
  );

DELETE FROM category WHERE slug = 'code_collaboration';

UPDATE category SET occupation_tags = '["general"]' WHERE slug = 'gaming';
