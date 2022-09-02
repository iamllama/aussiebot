SELECT twitch.disp_name as disp_name, modaction_twitch.platform_id, action, reason, at FROM modaction_twitch
	LEFT JOIN twitch ON twitch.platform_id = modaction_twitch.platform_id
	ORDER BY id DESC
	LIMIT 10