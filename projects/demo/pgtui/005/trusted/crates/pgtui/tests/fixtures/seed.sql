-- tests/fixtures/seed.sql — applied by pg_container() with testcontainers
-- `with_init_sql` (D-072). Spec: experiments/fixtures/README.md. fake_data.rs
-- must reproduce these rows cell-for-cell as text.
-- Idempotent-ish: drops everything it creates first.

DROP TABLE IF EXISTS public.customers, public.orders, public.empty_table, audit.events;
DROP SCHEMA IF EXISTS audit;

CREATE SCHEMA audit;

CREATE TABLE public.customers (id int PRIMARY KEY, name text, balance numeric(10,2), signup_date date, note text);
INSERT INTO public.customers VALUES
  (1,'Ada',10.50,'2024-01-05','vip'), (2,'Bob',250.00,'2024-02-10',NULL),
  (3,'Cyd',-3.25,'2024-03-15','refund'), (4,'Dee',99.99,'2024-04-20',NULL), (5,'Eve',250.00,'2024-05-25','tie');

CREATE TABLE public.orders (id int PRIMARY KEY, customer_id int, total numeric(10,2), status text);
INSERT INTO public.orders VALUES
  (1,1,5.00,'paid'),(2,1,7.50,'paid'),(3,2,100.00,'open'),(4,3,1.25,'refunded'),
  (5,4,49.99,'paid'),(6,5,200.00,'open'),(7,5,50.00,'paid');

CREATE TABLE public.empty_table (id int PRIMARY KEY);

CREATE TABLE audit.events (id bigint PRIMARY KEY, kind text, at timestamptz);
INSERT INTO audit.events VALUES
  (1,'login','2024-06-01T10:00:00Z'),(2,'query','2024-06-01T10:05:00Z'),(3,'logout','2024-06-01T10:30:00Z');
