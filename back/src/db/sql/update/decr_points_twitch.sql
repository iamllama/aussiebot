UPDATE twitch SET twitch_points = twitch_points - $2
  WHERE platform_id = $1 and twitch_points >= $2
    RETURNING *;