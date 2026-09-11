--
-- PostgreSQL database dump
--

\restrict atyu9jxHjm1tmCv2iwGUN4rGTe9bIVd0jm8n83tCkhOWTFp2wt1ITECepgo11tJ

-- Dumped from database version 15.19 (Debian 15.19-0+deb12u1)
-- Dumped by pg_dump version 15.19 (Debian 15.19-0+deb12u1)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'SQL_ASCII';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: assets; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.assets (
    host text NOT NULL,
    criticality text NOT NULL,
    owner_name text DEFAULT 'desconocido'::text NOT NULL,
    asset_function text DEFAULT 'desconocido'::text NOT NULL,
    tags text[] DEFAULT '{}'::text[] NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT assets_criticality_check CHECK ((criticality = ANY (ARRAY['critical'::text, 'high'::text, 'medium'::text, 'low'::text])))
);

--
-- Name: case_events; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.case_events (
    id bigint NOT NULL,
    case_id bigint NOT NULL,
    occurred_at timestamp with time zone NOT NULL,
    event_type text NOT NULL,
    severity text,
    title text NOT NULL,
    evidence jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

--
-- Name: case_events_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.case_events_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: case_events_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.case_events_id_seq OWNED BY public.case_events.id;

--
-- Name: soc_cases; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.soc_cases (
    id bigint NOT NULL,
    case_key text NOT NULL,
    status text DEFAULT 'open'::text NOT NULL,
    severity text NOT NULL,
    priority text NOT NULL,
    title text NOT NULL,
    host text NOT NULL,
    first_seen timestamp with time zone NOT NULL,
    last_seen timestamp with time zone NOT NULL,
    source_ips text[] DEFAULT '{}'::inet[] NOT NULL,
    alert_ids text[] DEFAULT '{}'::text[] NOT NULL,
    mitre_techniques text[] DEFAULT '{}'::text[] NOT NULL,
    confidence text DEFAULT 'low'::text NOT NULL,
    assigned_to text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT soc_cases_confidence_check CHECK ((confidence = ANY (ARRAY['high'::text, 'medium'::text, 'low'::text]))),
    CONSTRAINT soc_cases_priority_check CHECK ((priority = ANY (ARRAY['p1'::text, 'p2'::text, 'p3'::text, 'p4'::text]))),
    CONSTRAINT soc_cases_severity_check CHECK ((severity = ANY (ARRAY['critical'::text, 'high'::text, 'medium'::text, 'low'::text]))),
    CONSTRAINT soc_cases_status_check CHECK ((status = ANY (ARRAY['open'::text, 'investigating'::text, 'contained'::text, 'closed'::text])))
);

--
-- Name: soc_cases_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.soc_cases_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

--
-- Name: soc_cases_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.soc_cases_id_seq OWNED BY public.soc_cases.id;

--
-- Name: case_events id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.case_events ALTER COLUMN id SET DEFAULT nextval('public.case_events_id_seq'::regclass);

--
-- Name: soc_cases id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.soc_cases ALTER COLUMN id SET DEFAULT nextval('public.soc_cases_id_seq'::regclass);

--
-- Name: assets assets_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.assets
    ADD CONSTRAINT assets_pkey PRIMARY KEY (host);

--
-- Name: case_events case_events_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.case_events
    ADD CONSTRAINT case_events_pkey PRIMARY KEY (id);

--
-- Name: soc_cases soc_cases_case_key_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.soc_cases
    ADD CONSTRAINT soc_cases_case_key_key UNIQUE (case_key);

--
-- Name: soc_cases soc_cases_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.soc_cases
    ADD CONSTRAINT soc_cases_pkey PRIMARY KEY (id);

--
-- Name: case_events_case_time_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX case_events_case_time_idx ON public.case_events USING btree (case_id, occurred_at DESC);

--
-- Name: soc_cases_host_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX soc_cases_host_idx ON public.soc_cases USING btree (host, last_seen DESC);

--
-- Name: soc_cases_status_priority_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX soc_cases_status_priority_idx ON public.soc_cases USING btree (status, priority, last_seen DESC);

--
-- Name: case_events case_events_case_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.case_events
    ADD CONSTRAINT case_events_case_id_fkey FOREIGN KEY (case_id) REFERENCES public.soc_cases(id) ON DELETE CASCADE;

--
-- PostgreSQL database dump complete
--

\unrestrict atyu9jxHjm1tmCv2iwGUN4rGTe9bIVd0jm8n83tCkhOWTFp2wt1ITECepgo11tJ
