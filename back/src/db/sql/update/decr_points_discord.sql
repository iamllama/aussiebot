UPDATE discord SET discord_points = discord_points - $2
  WHERE platform_id = $1 and discord_points >= $2
    RETURNING *;