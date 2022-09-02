 SELECT modaction_discord.platform_id AS id,
        discord.disp_name AS name,
        modaction_discord.action as action,
        modaction_discord.reason as reason,
        modaction_discord.at as at
        FROM modaction_discord
          LEFT JOIN discord ON discord.platform_id = modaction_discord.platform_id;