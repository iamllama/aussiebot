UPDATE youtube SET youtube_points = youtube_points + $2
  WHERE disp_name = $1
    RETURNING *;