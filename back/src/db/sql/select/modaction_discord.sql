SELECT discord.disp_name as disp_name, modaction_discord.platform_id, action, reason, at FROM modaction_discord
	LEFT JOIN discord ON discord.platform_id = modaction_discord.platform_id
	ORDER BY id DESC
	LIMIT 10