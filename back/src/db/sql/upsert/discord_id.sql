INSERT INTO discord (platform_id, disp_name, discord_points) 
  VALUES ($1, $2, $3) 
  ON CONFLICT (platform_id) 
  DO UPDATE SET disp_name = $2, discord_points = discord.discord_points + excluded.discord_points
  RETURNING *;