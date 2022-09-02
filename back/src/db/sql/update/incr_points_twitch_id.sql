UPDATE twitch SET twitch_points = twitch_points + $2
  WHERE platform_id = $1
    RETURNING *;