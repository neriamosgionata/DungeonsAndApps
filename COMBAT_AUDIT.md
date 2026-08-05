# Combat System Audit

**Date**: 2026-08-05 (full re-audit — supersedes purged June 2026 audits)
**Scope**: `backend/src/routes/combat/` (44 files, ~11.6k LOC) + `backend/src/combat_engine/` (17 files, ~4.6k LOC) + `backend/src/ws.rs` + `web/src/routes/campaigns/[id]/initiative/+page.svelte` (FE listener cross-check)
**Method**: 5 parallel read-only deep-dive agents (engine · actions hot path · specials/spells · turn/tick/tactical · CRUD/WS/sync), all findings re-verified by the auditor in source. Full test suite baseline: **739 passed, 0 failed** (32 suites). `cargo check`: 0 errors, **1 warning** (`uid_for` never used).
**Context**: system grew ~12k → ~17.5k LOC since the June audits closed (rounds 1–9: Extra Attack, maneuvers, mounted combat, aura, pact magic, hazard overhaul, ready-action batching…). This audit is the first full pass over the post-growth surface.

---

## Executive Summary

| Severity | Count | Notes |
|----------|------:|-------|
| CRITICAL | 2 | Movement economy bypass; surprise never applies to first combatant |
| HIGH | 24 | Rules-engine violations, intel leaks, lost-update races, dead rules |
| MEDIUM | 35 | PHB edge cases, missing gates, sync gaps, race conditions |
| LOW | 27 | Nits, doc rot, cosmetic/validation issues |

**Verdict**: the combat core (damage resolution, temp HP, death saves in the main attack path, crit math, action-economy atomicity, transaction hygiene) is sound — 40+ mechanics verified clean. But the post-June growth introduced **systematic rule drift in secondary paths** (multiattack/spell/TWF/OA reactions bypass what the main attack path enforces), **two exploitable economy bypasses** (free movement, negative heal), and **several dead rules** (Help inverted, ranged-in-melee dis dead, shove distance wrong, flanking geometry wrong).

**Fix priority (top 10)**:
1. CRIT-1 free-move economy bypass — movement cost dropped on 2nd+ move per round
2. CRIT-2 surprise never consumed for turn-order-0 combatant + surprised can react
3. HIGH Help grants advantage to *attackers against* the ally (inverted)
4. HIGH OA always deals unarmed 1+STR (never the weapon)
5. HIGH concentration check = raw CON mod, ignores proficiency/bonuses
6. HIGH reaction negation (Shield etc.) doesn't undo death saves/concentration/sheet
7. HIGH counterspell is cosmetic (effects already applied) + 500 on homebrew spells
8. HIGH Rage grants attack advantage + damage bonus on all attacks (PHB: STR melee only)
9. HIGH aura applies to auto-faction enemy NPCs + paladin double-dips own CHA
10. HIGH lost-update race on concurrent HP writes (no row lock on target)

---

## CRITICAL (2)

### C-1. Movement economy bypass — every move after the first per round is free
- **Loc**: `backend/src/routes/combat/combatants/move_combatant.rs:65-84`
- **Bug**: `token_moved_round == Some(round)` gates everything. First move: `movement_used_ft += cost`, cap checked. Every subsequent move: `new_movement_used = movement_used` — the **cost is dropped** and the per-move cap check (line 79) compares stale `movement_used` against speed, which never fires once the first move wrote a real value. Verified trace: move 30 ft (charged, `movement_used_ft=30`, `token_moved_round=round`) → second drag anywhere on map, cost discarded, `movement_used_ft` stays 30. `token_moved_round` resets only at round start. Second bug in the same fn: `movement_used` is read at line 19 **before** the tx; the `for update` at line 115 re-locks but the UPDATE at 120-136 writes the stale-derived value — two concurrent first-moves each compute `0 + cost`, double move for one charge (the comment at 111-113 claims a WHERE check that does not exist).
- **Fix**: always `movement_used + cost`; keep `cost > effective_speed` only as single-move cap; re-read `movement_used_ft` after `for update` (or `update … where movement_used_ft = $old` returning count).

### C-2. Surprise never applies to the first combatant in the turn order; surprised creatures can take reactions
- **Loc**: `encounters/start.rs:65-73` (turn_index set, no tick) + `tick.rs:226-252` (surprise consumption lives only in `tick_effects`, run solely by `next_turn`/`prev_turn`/`goto_turn`) + `opportunity.rs:159-163` (reaction gate only checks `reaction_used`)
- **Bug**: at encounter start, `turn_index` = first combatant with full economy (start.rs resets all flags, never consumes surprise). `next_turn` 0→1 consumes surprise only for the combatant at index 1. The turn-order-0 combatant keeps full action/BA/movement for round 1 — and at the round wrap (N-1→0) their surprise is then wrongly consumed, eating their round-2 turn. Independently: the surprise consumption (tick.rs:226-252) sets `action_used/bonus_action_used/movement_used_ft` but **not `reaction_used`**, and no action endpoint checks the `surprised` condition — a surprised creature can Shield/OA in round 1 before its turn.
- **Fix**: run the surprise-consumption block for the active combatant inside `start()` (and after `goto_turn`); set `reaction_used = true` in the consumption SQL.

---

## HIGH (24)

### Rules-engine violations

### H-1. Concentration check = raw CON ability mod; ignores proficiency, save bonuses, overrides
- **Loc**: `combat_engine/resolvers/damage_type.rs:86-101`
- **Bug**: `let con_mod = ability_mod(target, "con")` → `1d20+{con_mod}`. A CON-proficient caster (CON 14, PB +4 → +6) rolls concentration at +2; a DC 10 pass drops 85% → 65%, on every attack/spell/damage event. `ComputedStats.save_mods` (used by `resolve_save`) never consulted; `saves_override` ignored.
- **Fix**: `concentration_check` should take `&ComputedStats` and use the con `save_mods` entry (+ War Caster / magic-resistance advantage).

### H-2. Help action is inverted — grants advantage to anyone attacking the helped ally
- **Loc**: `actions/economy/help.rs:31-42` + `combat_engine/resolvers/attack.rs:94-96`
- **Bug**: Help inserts `{"attack_advantage_against": true}` on `target_id`. The resolver reads that key from `target_stats` → `adv = true` for the **attacker** ("attacks vs this combatant get advantage" — the Dodge/Reckless key). Net: the helped ally is now *easier to hit* by everyone. Also the effect ticks at `target_turn_start` (1 round), not PHB "next attack roll".
- **Fix**: use `{"attack_advantage": true}` (attacker-side key at attack.rs:24) on the target.

### H-3. Opportunity attacks always deal unarmed damage (1 + STR), never the weapon
- **Loc**: `actions/economy/opportunity.rs:120-147`
- **Bug**: OA builds `AttackReq` with `weapon_id: None`, `attack_expression: None`, `damage_expression: None`, `damage_type: "bludgeoning"` → resolver falls to the unarmed default (`1+str`). A greatsword OA rolls 1+STR bludgeoning. Also no verification the target actually moved (endpoint callable any time vs any reachable target).
- **Fix**: look up attacker's equipped weapon; pass `weapon_id` + its damage type/expression.

### H-4. Rage grants advantage on ALL attack rolls (PHB: STR checks/saves only)
- **Loc**: `special/class_feature.rs:204-213` (modifier `{"attack_advantage": true}`) → `stats/compute.rs:586-587` → `resolvers/attack.rs:38` (OR'd into every attack) and Sneak-Attack eligibility (attack.rs:427)
- **Bug**: PHB p.48 — Rage gives advantage on Strength **checks** and Strength saves, never attack rolls. The handler message itself says "STR advantage" (class_feature.rs:238-240) — code does the opposite. Also feeds the auto-Sneak gate.
- **Fix**: use a `str_check_advantage` modifier consumed by skill-check resolution only; remove `attack_advantage`.

### H-5. Rage damage bonus applies to all attacks (ranged/spell/DEX melee)
- **Loc**: `resolvers/attack.rs:436-442`
- **Bug**: `raw_dmg = dmg_roll.total + attacker_stats.damage_bonus + …` — unconditional. PHB: rage bonus only on melee weapon attacks using Strength.
- **Fix**: gate `damage_bonus` on melee/STR weapon attacks.

### H-6. Ranged-within-5ft disadvantage and Sneak-Attack ally-adjacency are dead checks (wrong scale)
- **Loc**: `actions/combat/attack.rs:182-191` (`< 1.5`) and `attack.rs:455` (`< 1.5`)
- **Bug**: every other consumer (engine prone-check `d < 20.0` attack.rs:60, aura `×0.25`, hazards, opportunity `×0.25`) uses 5 ft = 20 percent-units of the map. `< 1.5` ≈ 0.4 ft: tokens in adjacent cells (20 apart) never qualify → both rules effectively dead for placed tokens. The ranged check also ignores hostility and `hp_current > 0` — adjacent ally or corpse imposes the disadvantage.
- **Fix**: `< 20.0` (or derive from `map_grid_size`), filter hostile side + `hp_current > 0`.

### H-7. Reaction negations (Shield/Parry/Deflect/Interception/Protection) don't undo the hit's side effects
- **Loc**: `actions/reactions.rs:101-126` (Shield restore) — no `sync_combatant_hp_to_sheet` anywhere in reactions.rs
- **Bug**: attack_apply commits death-save failures (attack_apply.rs:278-294), `alive=false`/instant death (:252-263), concentration deactivation (:246-250), rider dismount (:509-511), and the sheet HP sync — then publishes the reaction window. A Shield that negates a killing blow leaves the sheet with +1 death-save failure, broken concentration, and stale HP. Token and sheet permanently diverge.
- **Fix**: after a successful negation, re-sync the sheet and reverse the failure/concentration writes (or open the reaction window before commit).

### H-8. Aura of Protection applies to enemy NPCs and double-dips the paladin's own CHA
- **Loc**: `aura.rs:35-41` (only `faction == "hostile"` excluded; default faction is `'auto'`, which heal.rs:46-52 derives as enemy for NPCs) + `stats/compute.rs:271-279` (paladin 6+ folds own CHA into own `save_mods` unconditionally) + `aura.rs:43-54` (query includes the paladin itself; self-distance 0)
- **Bug**: (a) every default-faction enemy NPC standing near a paladin gets +CHA on all saves; (b) the paladin's own saves get CHA twice (own `save_mods` + `req.aura_bonus` from self in range).
- **Fix**: derive faction like heal.rs (`hostile`/`enemy`/`auto`+npc → excluded); exclude the paladin itself from the aura query (or drop the compute.rs fold).

### H-9. Battle Master maneuvers: guaranteed damage with no attack roll, no action cost, no trigger validation
- **Loc**: `special/class_feature.rs:953-1150`
- **Bug**: the trip/disarming/pushing/goading/menacing branch applies `apply_hp_damage(hp_cur, temp_hp, sd_roll)` directly — no roll vs AC, no action/BA/reaction consumed, riposte/sweeping triggers not validated. A fighter gets auto-hit superiority-die damage every round on top of a full action (only sweeping/riposte have an `atk` roll; the SD-damage branch runs for all).
- **Fix**: require a hit (or pending miss for riposte) + consume the Attack action (or add SD to a real attack roll).

### H-10. Spell and multiattack damage at 0 HP never record death saves / instant death
- **Loc**: `spells/cast.rs:647-650` (`instant_death` computed, never consumed) + `spells/apply.rs:242-370` + `special/multiattack.rs:196-282`
- **Bug**: attack path records failures + `alive=false` (attack_apply.rs:252-295); spell/multiattack paths only update `hp_current`/`temp_hp`. Fireball KO → `alive=true`, no failure. Multiattack also skips `pending_hits` (reactions can't respond to its hits), readied-action triggers, and the reaction window.
- **Fix**: reuse the attack_apply death-save/instant-death block and per-hit `pending_hits` in both paths.

### H-11. Counterspell has zero mechanical effect (spell resolves first) + 500s on homebrew spells
- **Loc**: `actions/reactions.rs:465-530` vs `spells/apply.rs:396-464`
- **Bug**: `apply_spell_outcome` commits the full effect (damage, conditions, slot decrement) and *then* publishes `reaction_window` post-commit. Counterspell only clears `spell_being_cast` — damage already landed, slot already spent. Countering is cosmetic. Separately, the level lookup is `select level::int from spells where slug = $1` (`fetch_one` → 500 for `campaign_spells` slugs, whole reaction rolls back).
- **Fix**: refund path on counter (restore HP, remove applied effects, refund slot) or pre-apply reaction window; fall back to `campaign_spells` for the level.

### H-12. Counterspell: no slot consumption, no spellcasting validation, client-supplied check
- **Loc**: `reactions.rs:465-530`
- **Bug**: `slot_level` is client-claimed; nothing decrements `sheet.slots`, no availability check (a 1st-level fighter counterspells at level 9 for free, forever). `ability_check_total` is client-supplied for the DC-10+level check. Also resolves against the spell's **base** level, not the slot level cast at (`spell_being_cast` stores slug only).
- **Fix**: consume the declared slot in-tx; server-roll the check; store `slug:level` in `spell_being_cast` and compare vs slot used.

### H-13. Help/Dodge-style modifiers: restrained-as-effect sets global save disadvantage
- **Loc**: `stats/compute.rs:611` vs `compute.rs:28`
- **Bug**: condition path sets `save_disadvantage_for("dex")` (correct, PHB p.292); the same condition arriving as an effect modifier (Web, Entangle, Ensnaring Strike) sets the global `stats.save_disadvantage` — STR/CON/INT/WIS/CHA saves also roll at disadvantage. Same creature, different saves depending on how the restraint arrived.
- **Fix**: use `save_disadvantage_for("dex")` in `apply_modifier` too.

### H-14. Client-supplied `attack_expression` bypasses all server-side advantage/disadvantage
- **Loc**: `actions/combat/attack.rs:556-558` → `resolvers/attack.rs:154-160`
- **Bug**: when `req.attack_expression` is set, the resolver uses it verbatim (only appends Precision Attack), skipping `effective_adv`/`effective_dis`/cover. A client sending `"1d20+9"` hits a Dodging, prone, frightened-in-LOS target with no penalty, and can't benefit from flanking/help.
- **Fix**: wrap custom expressions in `2d20kh1`/`2d20kl1` when effective adv/dis is set.

### Turn/tick layer

### H-15. `prev_turn` / `goto_turn` re-run the forward tick pipeline — damage re-applied
- **Loc**: `encounters/turns.rs:104-126` (prev_turn), `turns.rs:161-183` (goto_turn) → `tick_effects`
- **Bug**: jumping backward re-applies hazard damage, regen, condition ticks, and `blinded:N → N-1` decrements — a target jumped back to burns a full condition turn per backward jump; `goto_turn` to the same index twice re-deducts hazard damage. Comment claims goto is for "undo a misclick".
- **Fix**: skip hazard/regen/condition-tick when new turn order ≤ old (true reverse-tick).

### H-16. Readied actions live a full round past expiry and survive the owner's turn start
- **Loc**: `reactions.rs:769-777` (`expires_at_round = round + 1`) + `turns.rs:78-84` (`expires_at_round < new_round` cleared only on round transitions)
- **Bug**: set round 1 → expires 2; transition 1→2: `2 < 2` false → survives all of round 2 incl. past the owner's own turn start (the per-turn reset doesn't clear `readied_action`); still auto-triggers. Stale readied action can fire a full round late.
- **Fix**: clear `readied_action` at owner's turn start (and/or store `set_at_round + 1`, clear on transition at owner position).

### H-17. Tick hazard saves miss aura, per-ability disadvantage, auto-fail STR/DEX, advantages
- **Loc**: `tick.rs:315-356` (hand-rolled inline save) vs the full `resolvers/save.rs` used by the other 4 save paths
- **Bug**: per-turn hazard saves use only `save_mods` + global `stats.save_disadvantage`: no `aura_bonus` (allies gain nothing), no `save_disadvantage_abilities` (restrained target rolls DEX hazard saves flat), no auto-fail for paralyzed/stunned/unconscious, no Gnome Cunning/Danger Sense/magic-resistance advantage. Stale comment at tick.rs:326 claims "Exhaustion 1+ gives save disadvantage" (it's L3).
- **Fix**: call `resolve_save` with `aura_bonus` (same as overlay_damage).

### H-18. Cone/line overlays resolve as circles for damage — 30-ft cone ≈ full map
- **Loc**: `tick.rs:299-306` and `tactical/hazards.rs:92-97` — `match shape { "circle" => …, "cube"|"square" => …, _ => circle }`; `length_ft/width_ft` fetched but unused
- **Bug**: FE creates `shape:'cone'` and `'line'` (wall) zones; backend damages everything in a circle of `radius_ft×4%` (10-ft cone → 40% of map; null radius → 80%). Wall/hazard lines damage in a circle.
- **Fix**: implement cone sweep and line/rect containment; never fall back to circle.

### WS / sync / authz

### H-19. `combatants_join_batch` WS event never handled by the frontend
- **Loc**: `combatants/bulk.rs:285-291`; `web/src/routes/campaigns/[id]/initiative/+page.svelte:558`
- **Bug**: reload gate is `t.startsWith('combatant_')` — `"combatants_join_batch"` misses the prefix. Bulk adds (template spawns) leave every other client with a stale roster until an unrelated event fires.
- **Fix**: also match `t === 'combatants_join_batch'`.

### H-20. `prev_turn` / `goto_turn` WS events unhandled by the frontend — turn desync
- **Loc**: `turns.rs:249-252` (`"type":"prev_turn"`), `turns.rs:340-347` (`"type":"goto_turn"`); `+page.svelte:558` lists only `next_turn`
- **Bug**: after GM clicks prev/goto, other clients keep stale turn_index/round until a `combatant_*`/tick event happens to fire (often never).
- **Fix**: add both events to the reload condition.

### H-21. Notification body leaks HP/AC of hidden combatants to all members
- **Loc**: `combatants/create.rs:144-157`, `bulk.rs:254-272` — `"Init {} · HP {}/{} · AC {}"` built unconditionally
- **Bug**: adding a hidden ambusher broadcasts exact HP/AC via notification to every non-GM member — contradicts the masking in list.rs:33-45 and the closed M-WS4 leak.
- **Fix**: gate the body (or the notify) on `is_visible`.

### H-22. Combat→sheet sync is silent — character page never refreshes
- **Loc**: `actions/sync.rs:11-58` (no `ws::` anywhere)
- **Bug**: AGENTS.md §10.6 documents "Emits `character_updated` WS" — no sync path emits anything. Sheet HP is stale after every attack/damage/heal/death-save/hazard/regen until manual reload (character page reloads only on `character_updated`/`combatant_updates`, +page.svelte:240).
- **Fix**: publish `character_updated` after each sync (or have the character page listen to combatant_* events). Also true doc rot: fix AGENTS.md §10.6.

### H-23. Lost-update race on concurrent HP writes (no row lock on target)
- **Loc**: `actions/combat/attack_apply.rs:239-244`, `combat/damage.rs:80-100`, `combatants/update.rs:74-130`
- **Bug**: target HP computed from a pre-tx snapshot, then written with an **unconditional** UPDATE. Two monsters hitting the same player concurrently both write from stale snapshots — last commit wins, first hit's damage lost. GM HP patch races the same way. Only the *attacker* row is locked.
- **Fix**: `select … for update` on the target inside the tx, or optimistic `where hp_current = $old`.

### H-24. Cross-campaign character linkage on combatant create
- **Loc**: `combatants/create.rs:28-35,116-137`; `bulk.rs:77-119`
- **Bug**: the only character check is `select (sheet->>'alive')… where id = $1 and campaign_id = $2` with `dead = None → passes`. A character from another campaign passes (row absent), the FK (init.sql:343) doesn't scope to campaign, and every `sync_combatant_hp_to_sheet` then writes HP/AC/alive **into a foreign campaign's sheet**. `bulk.rs` has no alive check at all and never verifies character campaign membership.
- **Fix**: reject when the scoped row is absent (both paths).

---

## MEDIUM (35)

### Engine / resolvers
- **M-1. Petrified targets don't grant attackers advantage** — `resolvers/attack.rs:81-89` omits `petrified` from the adv block (PHB p.291). Add `|| target_stats.petrified`.
- **M-2. Polearm Master omits the spear** — `resolvers/polearm.rs:22`: `POLEARM_NAMES = ["glaive","halberd","quarterstaff"]`; PHB includes spear. Add `"spear"` (route gate `economy/polearm.rs:63` too).
- **M-3. TWF/polearm BA attacks bypass adv/dis and auto-crit** — `two_weapon_fight.rs:49`, `polearm.rs:56` roll bare `1d20+…`: poisoned/blinded attacker no penalty, prone/paralyzed target no adv/auto-crit, unlike the main path. Hoist a shared helper.
- **M-4. GWF reroll applies to non-melee damage, 1-handed weapons, and spells** — `attack.rs:372-385` gates only `!ranged && !thrown`; a 1H longsword or fire bolt gets rerolls. PHB: melee weapon wielded with two hands; also the reroll policy should be per-die forced, not "reroll once keep best".
- **M-5. Dueling +2 applies to unarmed strikes and spell attacks** — `attack.rs:423-431`: no weapon-present check; also no "no other weapons" check.
- **M-6. Off-hand/polearm BA hardcode `is_magical=false`** — `twf.rs:151`, `polearm.rs:141`: +1 off-hand deals 0 vs nonmagical immunity while the main hand hits. Derive from `weapon.attack_bonus > 0`.
- **M-7. Smite +1d8 undead/fiend not doubled on crit** — `attack.rs:488-506`: slot dice double, bonus d8 doesn't. Roll `2d8` when critical.
- **M-8. `load_snapshot` vs `load_snapshots_batch` disagree on `level_total`** — `load.rs:52` (column first) vs `load.rs:234` (sheet only) — same combatant, different level/proficiency/HP per loader. Align coalesce order.
- **M-9. Multiclass HP gives the level-1 max die to the first class in the array** — `stats/hp.rs:24-29`: only the starting class should get it; array order isn't guaranteed canonical.
- **M-10. Dual Wielder melee detection differs between AC paths** — `stats/ac.rs:66-73` (thrown counts as melee) vs `compute.rs:151-162` (doesn't): handaxe user gets +1 AC via one path only. Share one predicate.
- **M-11. Savage Attacks/Brutal Critical dice added on spell/ranged crits** — `attack.rs:395-419`: PHB requires melee weapon attacks. Gate on `!is_spell_attack && melee`.
- **M-12. Death-save failure recorded on zero-damage hits** — `attack_apply.rs:264-295` gates only on `hp_before ≤ 0 && hp_after ≤ 0`, not HP reduction: a fully-immune hit or temp-absorbed hit on a downed creature still increments failures (damage.rs:113-117 and fall.rs:106-110 gate on `damage_applied > 0` — inconsistent). Gate on `(hp_before - hp_after) > 0`.
- **M-13. Ranged crit at 0 HP only causes 1 failure** — `attack_apply.rs:273-277`: `is_melee` gate is wrong; any crit = 2 failures (PHB p.197). The melee-only rule is the 5-ft auto-crit, not the failure count.
- **M-14. Reckless Attack advantage applies only to the triggering attack** — `attack.rs:128-141` + `attack_apply.rs:435-446`: persisted effect carries only `attack_advantage_against`; the follow-up Extra Attack/BA rolls flat. Persist an attacker-side `attack_advantage`.
- **M-15. Negative heal drives HP below 0, bypassing every damage rule** — `combat/heal.rs:15` allows `amount ≥ -1000`; `heal.rs` resolver has no floor and no death-save/alive handling. "Heal" −1000 → permanent 0-HP state with zero death-save pressure. Reject negatives (or route through the damage pipeline).
- **M-16. Flanking geometry uses a cell size inconsistent with the codebase convention** — `tactical/positioning.rs:97-121`: `max_dist = cell_pct * 2.0` with `px_per_pct = 6.0` → ~16.7, but adjacent cells are 20 apart everywhere else. Flanking never triggers in the canonical position. Derive cell size from the shared convention.

### Actions / economy / reactions
- **M-17. Reactions refresh at round start for everyone, not at each combatant's own turn start** — `turns.rs:66-71,210-214`: a turn-15 creature gets a fresh reaction at round start (up to 14 turns early) and can react on two consecutive early turns across the boundary. Reset in the per-combatant turn-start reset.
- **M-18. Readied-action auto-trigger ignores `expires_at_round`** — `reactions.rs:593-608` query has no expiry filter. Add `coalesce((readied_action->>'expires_at_round')::int, 0) >= $round`.
- **M-19. Shield negation uses the raw `ac` column, not the computed AC the resolver used** — `reactions.rs:57-61,89`: hits between `ac_col+5` and `computed_ac+5` (Haste +2, effects) land but aren't negatable, and vice-versa with debuffs. Use the computed AC (persist `target_ac` in the pending hit).
- **M-20. Double-restore race: two reactions can consume the same pending hit** — `reactions.rs:63-80` (Shield) + `405-417` (Protection) + `333-347` (Interception): plain read-modify-write, no `where pending_hits = $old` guard → target healed 2× damage. Conditional pop required.
- **M-21. Shield/Parry/Deflect don't restore temp HP absorbed by the negated hit** — `reactions.rs:105-110,279-284,168-173`: restore = `hp_before - hp_after` only; if the hit cost temp, temp is lost anyway. Record temp delta in the pending hit and restore it.
- **M-22. Sneak Attack flag race** — `attack.rs:465-472` (pre-tx read) + `attack_apply.rs:158-165` (unconditional set): two concurrent attacks both apply sneak dice. Atomic `update … where sneak_attack_used_this_turn = false returning id`.
- **M-23. TWF main-hand light check inspects the wrong weapon** — `economy/twf.rs:104-118` picks the first other weapon in the sheet array (a longbow blocks shortsword+dagger). Add `main_hand_weapon_id` to the body.
- **M-24. Bonus-action Dash/Disengage/Hide have no class gate** — `economy/movement.rs:31-36,79-84`, `dodges.rs:38-42`, `contested.rs:127`: any member can BA-dash. Gate on Rogue 2+ (Cunning Action) / Monk 2+ (Step of the Wind).
- **M-25. `deal_damage` never validates the source combatant's encounter** — `combat/damage.rs:48-58`: source may live in another campaign's encounter; damage attributed cross-encounter. Verify shared encounter_id.
- **M-26. Grapple/escape/shove ties succeed** — `grapple.rs:88`, `escape.rs:96`: `>=` instead of `>`. On a tie the situation stays as it was.
- **M-27. Shove push distance = 5% of map (~1.25 ft), not 5 ft** — `shove.rs:118`: PHB pushes 5 ft = 20% (class_feature.rs:1113-1118 uses 60.0 = 15 ft correctly).
- **M-28. Multiattack: no hp/incapacitation gate, no adv/dis, no readied-action trigger, no reaction window** — `multiattack.rs:40-56,135-162,285-295`: a 0-HP stunned NPC can multiattack; all attacks roll flat vs AC (no prone/flanking/cover/walls); readied `target_attacks` never fire.

### Class features
- **M-29. Lay on Hands consumes no action** — `class_feature.rs:243-375`; heals + attacks same turn. Also wastes pool at full HP (`missing=0 → heal_amt=1`, `:334-336`).
- **M-30. Standalone class-feature Smite auto-hits with no attack roll/action cost** — `class_feature.rs:807-955`: guaranteed radiant damage, stackable with the (correct) attack-path smite. Gate on a melee hit (reuse pending_hits) or drop the endpoint.
- **M-31. Rage ends on unconscious only at the *next turn start*; the `rage` condition persists** — `tick.rs:97-107` + `cast.rs:235`: while downed mid-round, resistance/bonus/advantage stay live; the condition is never removed on the tick path (only the attack/damage path retains it), so a revived barbarian is permanently blocked from casting. Deactivate on damage-to-0 and remove the condition together.
- **M-32. Rage / Second Wind / Action Surge have no per-rest usage limits** — `class_feature.rs:74-242`: only Action Surge is capped (manual marker); a level-1 barbarian rages and Second Winds every round. Read/decrement `sheet.resources`.
- **M-33. Wild Shape consumes no bonus action** — `class_feature.rs:1482-1612`.
- **M-34. Flurry of Blows target has no same-encounter validation (missing target silently hits AC 12)** — `class_feature.rs:492-623`.
- **M-35. Hazard damage doesn't break concentration; grappler KO'd by damage doesn't release the grapple** — `tick.rs:281-373` (no concentration check), `conditions.rs:196-227` (release only via `add_condition`, not damage paths).

### CRUD / sync / WS (from CRUD audit)
- **M-36. `temp_hp` can never be lowered/cleared via PATCH** — `update.rs:94` (`case when $7 > temp_hp`): GM "clear temp" silently no-ops; damage on a temp-absorbing NPC does nothing visible. Allow explicit lower/zero (authoritative temp_hp or `clear_temp_hp` flag).
- **M-37. Player can teleport their token via PATCH (bypasses move_combatant speed/wall checks)** — `update.rs:50-52`: `token_x/token_y` are in the player cosmetic whitelist; `is_visible` too (player can hide own token). Route through move_combatant or master-only.
- **M-38. Combat event log unmasked** — `events.rs:51-55`: `delta_hp` + action text with exact damage served to every member (incl. hidden NPCs, other players' characters); CombatLog renders it without an isMaster gate. Mask amounts for non-owners.
- **M-39. WS leaks `target_ac`/`attack_total` campaign-wide** — `attack_apply.rs:532-548,557-580` (`reaction_window`, `combatant_attacks`): exact AC/attack-bonus intel for hidden NPCs, while list.rs masks `ac`. Restrict to the target's owner or drop the fields.
- **M-40. `pending_hits` / `last_hit_*` unmasked in the player list query** — `list.rs:42-45`: pending_hits JSONB carries attack_total/damage/hp_before/hp_after. Zero out for non-owner rows.
- **M-41. `emit_campaign_bulk` inserts notifications but never pushes WS** — `notifications.rs:218-269`: no live badge for bulk adds. Publish per-user after insert.
- **M-42. Hazard damage and regen paths never sync combatant→sheet** — `tick.rs:217-240,262-282`: sheet HP diverges after any hazard/regen turn. Batch-sync affected ids.
- **M-43. `effects_change` published via non-persisted `ws::publish`** — `effects.rs:315-318,434,436,473,497`: reconnecting clients miss effect changes (replay covers only persisted events). Switch to `publish_persist`.
- **M-44. Bulk combatant creation skips `hp_max_reduction`** — `bulk.rs:160-238` vs `create.rs:77-92`: same character added single vs bulk yields different combatant hp_max. Align.

---

## LOW (27)

- **L-1.** Custom attack expressions assume the first term is the d20 (`resolvers/attack.rs:263-267` + polearm/twf/save/death_save/skill_check): `"1d4+1d20+5"` reads the d4 as natural. Find the d20 term explicitly.
- **L-2.** `resolve_heal` (default) ignores exhaustion-4 halved max — `resolvers/heal.rs:5`; all in-repo callers use `resolve_heal_with_max` (latent footgun).
- **L-3.** `save.rs:57-65` auto-fail branch fabricates `natural_roll: 1, save_total: 1` — reports a rolled 1 for paralyzed STR/DEX saves. Report the actual die.
- **L-4.** Generic race labels ("Dwarf", "Elf") get no ability bonuses — `stats/abilities.rs:140-198` only has "mountain dwarf"/"hill dwarf" etc.
- **L-5.** Exhaustion speed halving rounds up — `compute.rs:224` `.ceil()`; PHB rounds down. 35 ft → 18 instead of 17.
- **L-6.** Auto-crit/prone "assume melee range" when either token has no position — `attack.rs:62,288-291`: paralyzed target across the map treated as adjacent. Only default when both unplaced.
- **L-7.** PAM BA: masters bypass the bonus-action cost — `economy/polearm.rs:88-98` (`if auth.role != Role::Master`).
- **L-8.** `fall` writes a bool into the text `note` column — `combat/fall.rs:140` (`"true"`/`"false"` stored, verified against live DB).
- **L-9.** `delayed_turn` flag never reset — `economy/delay.rs:52`; no `delayed_turn = false` anywhere.
- **L-10.** OA/TWF/PAM hits skip post-hit effects (death-save failures, pending_hits, instant-death writes, rider dismount) — extract a shared `apply_hit_effects`.
- **L-11.** Polearm BA event uses non-persisted `ws::publish` — absent from replay log.
- **L-12.** Long-range check skipped when already at disadvantage — `attack.rs:226-259` (`&& !dis`) lets out-of-range shots through.
- **L-13.** Ammo matching `contains("Arrow")` can decrement the wrong item — `ammo.rs:98`.
- **L-14.** `utility.rs:75` binds `target_id` to `target_combatant` without validation — garbage UUID → FK 500.
- **L-15.** Champion `crit_range` applies to spell attacks — `cast.rs:554-556`; Improved Critical is weapon attacks only.
- **L-16.** Metamagic SP decrement silently skipped on race (no `for update`) — `apply.rs:180-219`.
- **L-17.** Rage persistence uses "missing HP" proxy — `turns.rs:113-120`; damage healed within the round ends rage spuriously.
- **L-18.** `extra_attack_count` doc rot — `stats/abilities.rs:66-67`: comment claims summing, code correctly takes max.
- **L-19.** Twinned Spell unvalidated — `cast.rs:204`: twinning AoE or 1-target spells allowed.
- **L-20.** Grapple/shove size limits unimplemented — `grapple.rs:52-88`, `shove.rs:52-88`.
- **L-21.** No automatic death save at turn start for 0-HP combatants (GM-manual only) — `tick.rs`.
- **L-22.** Grapple batch-release misses timed `grappled:N` entries — `conditions.rs:212-217` `array_remove` exact-match only.
- **L-23.** Plant condition immunities incomplete; `petrified` doesn't break concentration — `conditions.rs:73-82,153-157`.
- **L-24.** `calculate_cover` counts unplaced tokens as blockers from map center — `positioning.rs:158-166`; require `token_on_map = true`.
- **L-25.** `notify_turn` fires for dead combatants — `notifications.rs:10-23`; filter `hp_current > 0`.
- **L-26.** CRUD: `update_combatant` sheet-sync block dead code (character HP/AC PATCH rejected earlier) — `update.rs:109-114,143-148`; validator allows negative `hp_current` → DB CHECK 500 (types.rs:20,36 — set `min = 0`); `use_action` toggles lack the `hp_current > 0` gate (action.rs:25-60); `ref_type=character` without `character_id` → 500 (create.rs:21-25); mount "already ridden" check not FOR UPDATE (mount.rs:33-41); ws.rs echoes the raw JWT subprotocol (ws.rs:214-219); user-channel events dropped on lag with no replay (ws.rs:178-183); player `notes`/`readied_action` unmasked (list.rs:42); `notify_turn` round-gate skew for prev/goto (notifications.rs:25-37); dead code `uid_for` (cargo check warning).

---

## Doc rot (AGENTS.md / comments contradict code)

- AGENTS.md §10.6: "Emits `character_updated` WS" — sync.rs emits nothing (H-22).
- AGENTS.md §10.7: "Rage … `attack_advantage: true`" documented as intended — PHB violation (H-4/H-5).
- AGENTS.md §10.7/M21: "All 4 save paths wire [aura]" — the tick hazard path doesn't (H-17).
- AGENTS.md §5.5: `pending_hits` replaced `last_hit_*` — both still live, and `last_hit_*` cleared at turn start; pending_hits not (checked: turns.rs resets both — doc partially stale).
- Comment at `combat/attack.rs:111-113` (move_combatant): "The check in WHERE ensures concurrent moves can't double-decrement" — false (C-1).
- Comment at `stats/abilities.rs:66-67`: Extra Attack "summed across classes" — code takes max (correct PHB); fix comment.
- Comment at `tick.rs:326`: "Exhaustion 1+ gives save disadvantage" — it's L3.

---

## Verified clean (re-tested this pass)

- **Damage core**: temp-HP-before-real-HP absorption (saturating, floor 0), resistance/vulnerability cancel ordering (immune > nonmagical-immunity > resist+vuln cancel > vuln ×2 > resist half), HAM −3 gate, crit dice doubling preserving flats, power attack ±, archery gate, crit_range from sheet.
- **Death saves (attack path)**: nat20 heal/reset, nat1 2 failures, 3-of-a-kind resolution, clamps, revive reset, massive-damage instant death with exhaustion-4 threshold.
- **Action economy**: all consume paths use `UPDATE … WHERE used=false RETURNING id`; Extra Attack counting serialized via `for update`; BA+action spell restriction atomic (`action_spell_level`/`bonus_action_spell_level`); turn-start reset covers the full field list.
- **Transaction hygiene**: every audited handler commits once; all WS publishes after `tx.commit()`; error paths roll back action consumption.
- **Cantrip scaling, pact magic (pact-first), ritual rejection mid-combat, components (V/S/M + focus), concentration one-at-a-time, spell-attack vs save spells, Evasion, range validation** — all correct in the main cast path.
- **Sneak Attack** (server-validated: advantage or ally-adjacent, no disadvantage, finesse/ranged, once/turn) — modulo M-14 rage and the M-6-scale adjacency issue.
- **Legendary/lair actions**: atomic increments, per-round resets at the creature's own turn start, 0-HP/incapacitation gates.
- **Stunning Strike, Second Wind, Uncanny Dodge (temp-HP-safe refund), smite slot consumption, superiority-die consumption** — correct where they exist.
- **Aura range math** (10/30 ft, ×0.25 conversion, unplaced = in range) — modulo the 5-cell-map convention (below) and H-8.
- **Timed conditions, regen, exhaustion ladder, overlay expiry, surprised atomic consumption (when it runs)** — correct in `tick_effects`.
- **Wall LOS segment intersection; token NaN clamps; sheet↔combatant effective-max convention (raw − reduction) with feedback-loop guard; delete cascade (effects cascade, events null, mount null, turn renumber in-tx); WS token_version re-check; replay seq via advisory lock.**
- **SQL hygiene**: no string interpolation, `&mut *tx` reborrow everywhere, explicit column lists, enum `::text` casts.
- **Test suite**: `cargo test` → 739 passed / 0 failed across 32 suites; `cargo check` 0 errors (1 dead-code warning: `uid_for`).

---

## Cross-cutting: the %-to-feet convention is map-width-dependent (MED by design)

`dist_ft = dist_pct × 0.25` ("1 cell = 5 ft = 20% of map") appears in attack.rs:242, cast.rs:530, aura.rs:82, tick.rs:296-298, hazards.rs:63-66, opportunity.rs:98 — but the FE computes feet from **pixels** (`ft/5 × grid_size` relative to actual map width). The backend convention only holds when the map renders exactly 5 cells wide; on a 20-cell map all backend distances undercount ~4-5× (ranged weapons never out of range, aura 10 ft covers half the map, hazard 10 ft damages 40% of the map). H-6/M-16 are symptoms. Fix requires storing/deriving map pixel width server-side (or a per-encounter cell-percent constant shared with the FE).

---

## Recommended fix order

| Sprint | Scope |
|--------|-------|
| A (CRIT) | C-1 movement economy (both bugs) · C-2 surprise (start + reactions) |
| B (rules HIGH) | H-1 concentration · H-2 Help · H-3 OA weapon · H-4/H-5 Rage · H-7 negation side effects |
| C (mechanic HIGH) | H-9 maneuvers · H-10 spell/multiattack death saves · H-11/H-12 counterspell · H-8 aura |
| D (turn/WS HIGH) | H-15 prev/goto tick · H-16 readied expiry · H-17 hazard saves · H-18 cone/line · H-19/H-20 WS listeners · H-21..H-24 leaks/races/sync |
| E (MED sweep) | M-1..M-35 rules + class features + M-36..M-44 CRUD/WS |
| F (LOW + docs) | L-1..L-26 + doc-rot fixes in AGENTS.md |

Each fix must ship with a regression test (existing combat suites: `combat_integration.rs`, `combat_engine_unit.rs`, `combat_advanced.rs`, `combat_coverage_jun2026.rs`).
