INSERT INTO twitch (platform_id, disp_name, twitch_points) 
  VALUES ($1, $2, $3) 
  ON CONFLICT (platform_id) 
  DO UPDATE SET disp_name = $2, twitch_points = twitch.twitch_points + excluded.twitch_points
  RETURNING *;