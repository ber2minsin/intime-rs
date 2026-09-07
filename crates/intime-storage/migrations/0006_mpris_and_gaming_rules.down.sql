-- Reverse 0006 patches (best-effort). Seed rows from 0005 remain.

DELETE FROM activity_rule
WHERE source = 'seed'
  AND (
    (match_field = 'automation_id' AND match_op = 'contains' AND lower(pattern) IN (
      'brave','firefox','chrome','chromium','edge','vivaldi','opera','spotify','vlc','mpv','jellyfin','plex','rhythmbox'
    ))
    OR (match_field = 'product_name' AND match_op = 'contains' AND lower(pattern) IN (
      'league of legends','valorant','riot client','osu!','counter-strike','dota','path of exile',
      'world of warcraft','ubisoft connect','rockstar games','parsec','ppsspp','duckstation'
    ))
    OR (match_field = 'window_title' AND match_op = 'contains' AND lower(pattern) IN (
      'league of legends','valorant','counter-strike','dota 2','minecraft','roblox','fortnite',
      'path of exile','lutris','heroic games'
    ))
    OR (match_field = 'url' AND match_op = 'contains' AND lower(pattern) IN (
      'steamdb.info','protondb.com','nexusmods.com','curseforge.com','modrinth.com','op.gg',
      'tracker.gg','chess.com','lichess.org','howlongtobeat.com','store.epicgames.com','itch.io'
    ))
    OR (match_field = 'executable_path' AND match_op = 'contains' AND lower(pattern) IN (
      'steamapps','heroic/games','riot-client','battle.net'
    ))
    OR (match_field = 'company' AND match_op = 'contains' AND lower(pattern) IN (
      'riot games','blizzard','electronic arts','ubisoft','rockstar games','nintendo'
    ))
    OR (match_field = 'aumid' AND match_op = 'contains' AND pattern = 'com.valvesoftware.Steam')
  );

UPDATE activity_rule
SET priority = 140
WHERE match_field = 'focused_control_type'
  AND match_op = 'equals'
  AND lower(pattern) = 'mpris';
