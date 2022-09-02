INSERT INTO modaction_youtube (platform_id, action, reason) 
  VALUES ($1, $2, $3) 
  RETURNING *;