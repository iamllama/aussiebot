UPDATE youtube SET youtube_points = youtube_points + $2
  WHERE platform_id = $1
    RETURNING *;