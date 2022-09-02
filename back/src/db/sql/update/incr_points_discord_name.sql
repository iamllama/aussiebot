UPDATE discord SET discord_points = discord_points + $2
  WHERE disp_name = $1
    RETURNING *;