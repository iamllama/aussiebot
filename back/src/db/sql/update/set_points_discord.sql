UPDATE discord SET discord_points = $2
  WHERE disp_name = $1 and discord_points < $2
    RETURNING *;