UPDATE youtube SET youtube_points = $2
  WHERE disp_name = $1 and youtube_points < $2
    RETURNING *;