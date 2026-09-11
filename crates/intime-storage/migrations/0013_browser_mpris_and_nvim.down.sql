DELETE FROM activity_rule
WHERE source = 'seed'
  AND (
    (match_field = 'automation_id' AND match_op = 'contains'
      AND lower(pattern) IN ('brave','chrome','chromium','firefox','msedge','edge','vivaldi','opera')
      AND notes = 'Browser MPRIS → streaming')
    OR (match_field = 'window_title' AND match_op = 'contains'
      AND lower(pattern) IN ('nvim ','nvim','neovim')
      AND notes LIKE 'Neovim%')
  );
