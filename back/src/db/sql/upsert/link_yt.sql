INSERT INTO link_yt (id, discord_id) 
  VALUES ($1, $2) 
  ON CONFLICT (id) 
  DO UPDATE SET discord_id = $2
  RETURNING *;