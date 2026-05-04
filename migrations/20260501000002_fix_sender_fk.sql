-- Change messages FKs from CASCADE to SET NULL so deleting a user
-- preserves chat history instead of wiping it.

alter table messages drop constraint messages_sender_id_fkey;
alter table messages alter column sender_id drop not null;
alter table messages add constraint messages_sender_id_fkey
    foreign key (sender_id) references users(id) on delete set null;

alter table messages drop constraint messages_recipient_id_fkey;
alter table messages add constraint messages_recipient_id_fkey
    foreign key (recipient_id) references users(id) on delete set null;
