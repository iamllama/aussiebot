INSERT INTO youtube (platform_id, disp_name, youtube_points) 
  VALUES ($1, $2, $3) 
  ON CONFLICT (platform_id) 
  DO UPDATE SET disp_name = $2, youtube_points = youtube.youtube_points + excluded.youtube_points
  RETURNING *;