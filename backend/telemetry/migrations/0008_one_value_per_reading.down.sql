alter table readings add column payload jsonb not null default '{}'::jsonb;
