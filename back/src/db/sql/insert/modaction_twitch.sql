INSERT INTO modaction_twitch (platform_id, action, reason) 
  VALUES ($1, $2, $3) 
  RETURNING *;