INSERT INTO discord (platform_id, time_watched, last_seen) 
  VALUES ($1, $2, $3) 
  ON CONFLICT (platform_id) 
  DO UPDATE SET time_watched = $2, last_seen = $3
  RETURNING *;