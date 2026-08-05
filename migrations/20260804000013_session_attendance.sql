-- Session attendance (who was present when this happened, recap accuracy).
create table if not exists session_attendance (
    session_id uuid not null references campaign_sessions(id) on delete cascade,
    user_id    uuid not null references users(id) on delete cascade,
    primary key (session_id, user_id)
);
