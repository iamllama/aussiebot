CREATE TABLE public.twitch
(
    platform_id character varying NOT NULL,
    disp_name character varying,
    twitch_points integer DEFAULT 0,
    time_watched integer DEFAULT 0,
    last_seen timestamp with time zone DEFAULT now(),
    PRIMARY KEY (platform_id)
);

ALTER TABLE IF EXISTS public.twitch
    OWNER to aussiebot;

GRANT ALL ON TABLE public.twitch TO aussiebot;