UPDATE twitch SET twitch_points = $2
  WHERE disp_name = $1 and twitch_points < $2
    RETURNING *;