CREATE TABLE public.modaction
(
    id serial NOT NULL,
    platform_id character varying NOT NULL,
    action character varying NOT NULL,
    reason character varying,
    at timestamp with time zone DEFAULT now(),
    PRIMARY KEY (id)
);

ALTER TABLE IF EXISTS public.modaction
    OWNER to aussiebot;

GRANT ALL ON TABLE public.modaction TO aussiebot;