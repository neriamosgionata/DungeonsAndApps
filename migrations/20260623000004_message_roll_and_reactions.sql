-- Chat enhancements: inline dice rolls + emoji reactions.

-- Server-authoritative roll payload attached to a chat message when the
-- sender posts a "/roll <expr>" message. NULL for ordinary text messages.
alter table messages add column roll_result jsonb;

create table message_reactions (
    message_id uuid not null references messages(id) on delete cascade,
    user_id    uuid not null references users(id) on delete cascade,
    emoji      text not null,
    created_at timestamptz not null default now(),
    primary key (message_id, user_id, emoji)
);

create index on message_reactions(message_id);
