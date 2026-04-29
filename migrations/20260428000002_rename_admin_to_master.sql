-- App-wide role renamed: admin → master (aligning with in-app terminology).
-- The membership_role enum already has 'master' (campaign master) — it is a
-- separate type, no conflict.

alter type user_role rename value 'admin' to 'master';
