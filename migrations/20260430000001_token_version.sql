-- Token versioning for logout / password-change invalidation.
-- Incrementing this column revokes all previously-issued JWTs.
alter table users add column if not exists token_version int not null default 0;

create index if not exists idx_users_token_version on users(id, token_version);
