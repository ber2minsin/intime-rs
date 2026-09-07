-- Backend finish: coding/collaboration category, GitHub routing, gaming launchers,
-- SWE docs sites, desktop accounting. Safe on existing DBs.

INSERT OR IGNORE INTO category (slug, name, description, occupation_tags, source)
VALUES (
  'code_collaboration',
  'Code collaboration',
  'GitHub/GitLab/Bitbucket code, PRs, and repos (not project boards)',
  '["developer"]',
  'seed'
);

UPDATE category SET occupation_tags = '["general","gamer"]'
WHERE slug = 'gaming';

-- GitHub / GitLab code paths beat weak project_mgmt@75 and browsing.
INSERT OR IGNORE INTO activity_rule (category_id, match_field, match_op, pattern, priority, source, notes) VALUES
((SELECT id FROM category WHERE slug='code_collaboration'), 'url', 'contains', 'github.com/', 128, 'seed', 'GitHub code/PR'),
((SELECT id FROM category WHERE slug='code_collaboration'), 'window_title', 'contains', 'github.com/', 122, 'seed', 'GitHub in title'),
((SELECT id FROM category WHERE slug='code_collaboration'), 'window_title', 'contains', 'pull request', 118, 'seed', 'PR title'),
((SELECT id FROM category WHERE slug='code_collaboration'), 'window_title', 'contains', ' · github', 120, 'seed', 'GitHub tab chrome'),
((SELECT id FROM category WHERE slug='code_collaboration'), 'url', 'contains', 'gitlab.com/', 126, 'seed', 'GitLab'),
((SELECT id FROM category WHERE slug='code_collaboration'), 'url', 'contains', 'bitbucket.org/', 124, 'seed', 'Bitbucket'),
((SELECT id FROM category WHERE slug='code_collaboration'), 'url', 'contains', 'codeberg.org/', 120, 'seed', 'Codeberg');

-- Keep boards/issues tooling in project_mgmt at higher priority than generic github.com.
INSERT OR IGNORE INTO activity_rule (category_id, match_field, match_op, pattern, priority, source, notes) VALUES
((SELECT id FROM category WHERE slug='project_mgmt'), 'url', 'contains', 'github.com/issues', 135, 'seed', 'GitHub issues list'),
((SELECT id FROM category WHERE slug='project_mgmt'), 'url', 'contains', 'github.com/projects', 135, 'seed', 'GitHub projects'),
((SELECT id FROM category WHERE slug='devops'), 'url', 'contains', 'github.com/actions', 135, 'seed', 'GitHub Actions');

-- Developer docs / references (beat browsing).
INSERT OR IGNORE INTO activity_rule (category_id, match_field, match_op, pattern, priority, source, notes) VALUES
((SELECT id FROM category WHERE slug='research'), 'url', 'contains', 'docs.rs/', 115, 'seed', 'docs.rs'),
((SELECT id FROM category WHERE slug='research'), 'url', 'contains', 'doc.rust-lang.org', 115, 'seed', 'Rust docs'),
((SELECT id FROM category WHERE slug='research'), 'url', 'contains', 'developer.mozilla.org', 115, 'seed', 'MDN'),
((SELECT id FROM category WHERE slug='research'), 'url', 'contains', 'stackoverflow.com', 112, 'seed', 'Stack Overflow'),
((SELECT id FROM category WHERE slug='research'), 'url', 'contains', 'stackexchange.com', 110, 'seed', 'Stack Exchange'),
((SELECT id FROM category WHERE slug='research'), 'window_title', 'contains', 'stack overflow', 108, 'seed', 'SO title'),
((SELECT id FROM category WHERE slug='research'), 'url', 'contains', 'learn.microsoft.com', 110, 'seed', 'MS Learn'),
((SELECT id FROM category WHERE slug='research'), 'url', 'contains', 'pkg.go.dev', 110, 'seed', 'Go packages');

-- Gaming launchers + stores (beat browsing / unknown).
INSERT OR IGNORE INTO activity_rule (category_id, match_field, match_op, pattern, priority, source, notes) VALUES
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'steam', 130, 'seed', 'Steam'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'epicgameslauncher', 130, 'seed', 'Epic'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'epic games', 130, 'seed', 'Epic'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'riotclient', 130, 'seed', 'Riot Client'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'riot client', 130, 'seed', 'Riot Client'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'battle.net', 128, 'seed', 'Battle.net'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'agent', 125, 'seed', 'Battle.net Agent'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'ea desktop', 128, 'seed', 'EA App'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'ea app', 128, 'seed', 'EA App'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'ubisoftconnect', 128, 'seed', 'Ubisoft'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'ubisoft connect', 128, 'seed', 'Ubisoft'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'gog galaxy', 125, 'seed', 'GOG'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'heroic', 125, 'seed', 'Heroic'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'lutris', 125, 'seed', 'Lutris'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'legendary', 120, 'seed', 'Legendary'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'bottles', 115, 'seed', 'Bottles'),
((SELECT id FROM category WHERE slug='gaming'), 'product_name', 'contains', 'protonup', 110, 'seed', 'ProtonUp'),
((SELECT id FROM category WHERE slug='gaming'), 'aumid', 'contains', 'steam', 130, 'seed', 'Steam aumid'),
((SELECT id FROM category WHERE slug='gaming'), 'aumid', 'contains', 'riot', 128, 'seed', 'Riot aumid'),
((SELECT id FROM category WHERE slug='gaming'), 'executable_path', 'contains', 'steamapps', 135, 'seed', 'Steam apps path'),
((SELECT id FROM category WHERE slug='gaming'), 'executable_path', 'contains', 'Epic Games', 130, 'seed', 'Epic path'),
((SELECT id FROM category WHERE slug='gaming'), 'executable_path', 'contains', 'riot-client', 130, 'seed', 'Riot path'),
((SELECT id FROM category WHERE slug='gaming'), 'executable_path', 'contains', 'Battle.net', 128, 'seed', 'Battle.net path'),
((SELECT id FROM category WHERE slug='gaming'), 'window_title', 'contains', 'steam', 100, 'seed', 'Steam title'),
((SELECT id FROM category WHERE slug='gaming'), 'window_title', 'contains', 'epic games', 100, 'seed', 'Epic title'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'store.steampowered.com', 120, 'seed', 'Steam store'),
((SELECT id FROM category WHERE slug='gaming'), 'url', 'contains', 'store.epicgames.com', 120, 'seed', 'Epic store');

-- Desktop accounting.
INSERT OR IGNORE INTO activity_rule (category_id, match_field, match_op, pattern, priority, source, notes) VALUES
((SELECT id FROM category WHERE slug='finance_accounting'), 'product_name', 'contains', 'gnucash', 120, 'seed', 'GnuCash'),
((SELECT id FROM category WHERE slug='finance_accounting'), 'product_name', 'contains', 'quicken', 120, 'seed', 'Quicken'),
((SELECT id FROM category WHERE slug='finance_accounting'), 'product_name', 'contains', 'wave', 100, 'seed', 'Wave'),
((SELECT id FROM category WHERE slug='finance_accounting'), 'url', 'contains', 'waveapps.com', 120, 'seed', 'Wave'),
((SELECT id FROM category WHERE slug='finance_accounting'), 'url', 'contains', 'books.zoho.com', 120, 'seed', 'Zoho Books'),
((SELECT id FROM category WHERE slug='finance_accounting'), 'url', 'contains', 'freshbooks.com', 120, 'seed', 'FreshBooks');
