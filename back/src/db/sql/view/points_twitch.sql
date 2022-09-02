 SELECT youtube.platform_id AS youtube_id,
    discord.platform_id AS discord_id,
    twitch.platform_id AS twitch_id,
    youtube.youtube_points,
    discord.discord_points,
    twitch.twitch_points
   FROM twitch
     LEFT JOIN link_tw ON twitch.platform_id = link_tw.id
     LEFT JOIN discord ON discord.platform_id = link_tw.discord_id
     LEFT JOIN link_yt ON discord.platform_id = link_yt.discord_id
     LEFT JOIN youtube ON youtube.platform_id = link_yt.id;