SELECT last_seen, time_watched FROM youtube WHERE platform_id = $1 FOR UPDATE;