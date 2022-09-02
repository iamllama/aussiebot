UPDATE youtube SET youtube_points = youtube_points - $2
  WHERE platform_id = $1 and youtube_points >= $2
    RETURNING *;