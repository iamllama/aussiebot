INSERT INTO modaction_discord (platform_id, action, reason) 
  VALUES ($1, $2, $3) 
  RETURNING *;