CREATE TABLE public.link_yt
(
    id character varying NOT NULL,
    discord_id character varying,
    PRIMARY KEY (id)
);

ALTER TABLE IF EXISTS public.link_yt
    OWNER to aussiebot;

GRANT ALL ON TABLE public.link_yt TO aussiebot;