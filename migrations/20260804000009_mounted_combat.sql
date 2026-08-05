-- Mounted combat: rider → mount link (PHB p.198). One mount, one rider
-- (Mounted Combatant feat not modeled). Moving the mount moves the rider;
-- a mount reduced to 0 HP auto-dismounts its rider.
alter table combatants
    add column if not exists mounted_on uuid null references combatants(id) on delete set null;
