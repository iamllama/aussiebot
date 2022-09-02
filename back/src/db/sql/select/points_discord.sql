 SELECT youtube.platform_id AS youtube_id,
    discord.platform_id AS discord_id,
    twitch.platform_id AS twitch_id,
    youtube.youtube_points,
    discord.discord_points,
    twitch.twitch_points
   FROM discord
     LEFT JOIN link_yt ON discord.platform_id = link_yt.discord_id
     LEFT JOIN link_tw ON discord.platform_id = link_tw.discord_id
     LEFT JOIN youtube ON youtube.platform_id = link_yt.id
     LEFT JOIN twitch ON twitch.platform_id = link_tw.id
  	WHERE discord.platform_id = $1;