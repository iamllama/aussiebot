SELECT youtube.disp_name as disp_name, modaction_youtube.platform_id, action, reason, at FROM modaction_youtube
	LEFT JOIN youtube ON youtube.platform_id = modaction_youtube.platform_id
	ORDER BY id DESC
	LIMIT 10