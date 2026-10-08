//! A defeated house's end: `HouseClass::Update`'s multiplayer defeat gate,
//! `HouseClass::Blowup_All` and `MPlayer_Defeated`.
//!
//! Native owner: `HouseClass`. In every non-campaign game, once per house per
//! frame in HouseClass::Array order (`0x004F8E86..0x004F8F82`): a house that
//! is not defeated, not `MultiplayPassive=` and past frame zero, and whose
//! counts (`house_tracking`) say it holds nothing, has every object it
//! originally owns blown up (`Blowup_All @ 0x004FC6D0`) and is then marked
//! defeated (`MPlayer_Defeated @ 0x004FC0B0`, its only caller). With the retail
//! `ShortGame=yes`, losing the last counted building while no `BaseUnit=` is
//! tracked destroys the whole remaining army in that one call.
//!
//! Blowup_All walks TechnoClass::Array, limbo objects included, re-reading the
//! count each step. An object whose original owner
//! (`TechnoClass::GetOriginalOwner @ 0x0070F820`) is the house and that the
//! house still owns dies; one another house mind-controls is instead handed to
//! the Civilian-side house when its controller's CaptureManager finds one
//! (`SetOriginalOwnerToCivilian @ 0x00472330`), and dies only when none
//! exists. An object being erased first has its Temporal chain released
//! (`0x0071AD40`). Each dies through its own ReceiveDamage with its Health as
//! `C4Warhead=` damage, no attacker, ignoring defenses and passenger escape
//! (vtable `+0x16C`, called at `0x004FC766`): no kill credit, no survivors.
//!
//! Scenario draws (read, not executed): none in the gate or the sweep's own
//! code; each death draws in its own receiver (death sounds, debris, death
//! anims), in array order. The Temporal release idles the released attackers
//! (ClearLinkedList); VERA queues those idles, so any draw they make comes at
//! the idle's turn.
//!
//! Evidence: `tools/spatial_oracle/house_blowup_all.py` runs the original
//! Blowup_All with GetOriginalOwner, CaptureManager GetOriginalOwner and
//! SetOriginalOwnerToCivilian (10 cases; the Civilian side lookup is
//! supplied); `house_defeat_gate.py` the gate (21 cases). Control flow and
//! ordering read from the disassembly.
//!
//! RESIDUALS:
//! - Between the gate and the strategy tick, every eighth frame, native
//!   springs event 8 ("any event") on each of the house's tags, last to
//!   first (`0x004F8F87..0x004F8FBC`: the list at House+0x3C, its count at
//!   +0x48, `0x006E53A0`); VERA's trigger runtime keeps no tags on houses.
//!   Trigger: a map trigger attached to a house, mostly in campaigns.
//!   Effect: that trigger's "any event" never springs. Later owner: the
//!   trigger subsystem.
//! - TechnoClass::Array order stands on stable-id order (construction order),
//!   which matches for every source VERA constructs in native order.
//! - Slave release: a Slave Miner killed with no attacker hands its slaves on
//!   the map to the Civilian-side house and UnInits those in limbo (the
//!   ReceiveDamage death arm's `0x006B0AE0` call at `0x00702065`,
//!   `sim::slave_manager`). A master constructed before its slaves (its own
//!   constructor builds them) is reached first, and the sweep then skips
//!   them, as they no longer belong to the defeated house; a refinery
//!   deployed from a Slave Miner comes after the slaves it took over, so the
//!   sweep kills those first as the defeated house's own, in the same
//!   TechnoClass::Array order.
//! - The IsToDie path (`Flag_To_Die @ 0x004FC980`: DESTRUCT, REMOVEPLAYER, a
//!   last human's EXIT) has no VERA producer; offline skirmish cannot reach
//!   it.
//! - The trigger-held original owner (`+0x2CC`/`+0x2E0`,
//!   HouseClass::TransferUnitsTo) is not ported.
//! - MPlayer_Defeated's local-player branch (`0x004FC1E7..0x004FC307`: the
//!   whole-map reveal `0x00577F30` and the defeat UI), its capture-the-flag
//!   cleanup (Flag_Remove `0x004FBE40` at `0x004FC112..0x004FC15E`, Scenario
//!   flag `0x10`), the Harvester-Truce UnInit loop (`0x004FC163`, Scenario flag
//!   `0x800`) and Computer_Paranoid (`0x00501640`) are not ported; the flags
//!   are off in stock skirmish. Of the local branch only the map-clear byte is
//!   kept (see `mplayer_defeated`). Trigger: the local player is defeated.
//!   Effect: the shroud stays in place for the loser.

use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::combat::{EntityDamageEvent, RAD_NO_ATTACKER, ReceiverCallFlags};
use crate::sim::intern::InternedId;
use crate::sim::world::{SimSoundEvent, Simulation};

impl Simulation {
    /// The house rung with its defeat pass: see [`Self::house_rung`].
    #[cfg(test)]
    pub(super) fn check_defeat(
        &mut self,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
    ) {
        self.house_rung(rules, registry, true);
    }

    /// The house rung's per-house steps in HouseClass::Array order: each
    /// house's team creation (`0x004F8A00..0x004F8B08`,
    /// `sim::ai_team_creation`), its defeat gate (see the module doc), then
    /// its strategy tick (`0x004F8FBE..0x004F9032`, `sim::house_strategy`) and
    /// production choices (`0x004F9038..0x004F9265`, `sim::ai_base_building`);
    /// then the game-over scan and the result timers. Without `defeat_pass`
    /// (VERA's first tick) the defeat gates and the game-over scan are
    /// skipped.
    pub(super) fn house_rung(
        &mut self,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
        defeat_pass: bool,
    ) {
        let outcome_tick = self.session.tick.saturating_add(1);
        let savour_frames = crate::rules::ruleset::savour_delay_frames(
            rules
                .map(|rules| rules.general.savour_delay_minutes)
                // RulesClass__Constructor @ 0x00665650 stores the exact f64
                // default 0.03 before any optional INI ReadDouble override.
                .unwrap_or(0.03),
        );
        // 0x004F8E86..0x004F8EB7: not a campaign, past frame zero.
        let defeat_gate =
            defeat_pass && self.session.game_mode_nonzero && (self.session.binary_frame as i32) > 0;
        // The interner resolves names case-insensitively, as native type
        // lookups do.
        let base_units = rules.map_or([None; 3], |rules| {
            std::array::from_fn(|slot| {
                rules
                    .general
                    .base_unit_types
                    .get(slot)
                    .and_then(|name| self.interner.get(name))
            })
        });
        let build_refinery_2 = rules
            .and_then(|rules| rules.build_refinery_types.get(2))
            .and_then(|name| self.interner.get(name));
        // Logic55B68D reloads the live House count after each complete body.
        // A House's power/radar and activation must precede its own teams,
        // EVA, defeat and production; no global half-body sweeps intervene.
        let mut index = 0;
        while index < self.session.house_order.len() {
            let owner = self.session.house_order[index];
            if let Some(rules) = rules {
                self.assess_house_derived_state(owner, rules);
            }
            self.update_house_anger_and_activation(owner, rules);
            if let Some(power) = self.power_states.get_mut(&owner) {
                power.clamp_negative_totals();
            }
            if let Some(rules) = rules {
                crate::sim::ai_team_creation::update_team_creation(self, rules, owner);
                crate::sim::house_eva::update_house_eva(self, rules, owner);
            }
            if defeat_gate && self.house_holds_nothing(owner, &base_units, build_refinery_2) {
                if let Some(rules) = rules {
                    self.house_blowup_all(owner, rules, registry);
                }
                self.mplayer_defeated(owner, outcome_tick, savour_frames);
            }
            if let Some(rules) = rules {
                crate::sim::house_strategy::update_strategy(self, rules, owner, registry);
                crate::sim::ai_base_building::update_production_choices(
                    self, rules, owner, registry,
                );
            }
            if let Some(house) = self.houses.get_mut(&owner) {
                house.release_repair_latch(self.session.binary_frame);
                #[cfg(test)]
                if self
                    .house_update_append_after_test
                    .is_some_and(|(after, _)| after == owner)
                {
                    let (_, appended) = self
                        .house_update_append_after_test
                        .take()
                        .expect("matched append");
                    self.session.house_order.push(appended);
                }
            }
            index += 1;
        }
        if !defeat_pass {
            return;
        }

        // Check if all remaining alive houses are mutually allied → game over.
        // The native alive scan counts only houses that are neither defeated nor
        // passive; the Civilian/JP houses present in every skirmish own map
        // objects forever, so including them would keep the alive set above one
        // and the victory screen would never appear.
        let alive: Vec<InternedId> = self
            .houses
            .iter()
            .filter(|(_, h)| !h.is_defeated && !h.multiplay_passive)
            .map(|(k, _)| *k)
            .collect();

        // VERA-internal developer policy, gamemd equivalent UNCHECKED: an
        // authored solo sandbox must not create a victory state or EVA merely
        // for being the only contender. Keep accepted explicit outcomes and
        // their timers below independent of this automatic-creation gate.
        let automatic_victory_allowed = self.contending_house_count() > 1;
        if automatic_victory_allowed && alive.len() == 1 {
            // Last player standing.
            if let Some(h) = self.houses.get_mut(&alive[0])
                && h.flag_to_win(outcome_tick, savour_frames)
            {
                self.sound_events.push(SimSoundEvent::MatchOutcome {
                    owner: alive[0],
                    kind: crate::sim::house_state::HouseOutcomeKind::Victory,
                });
            }
        } else if automatic_victory_allowed && !alive.is_empty() {
            // O(n^2) mutual-alliance check. Native alliance is directional — each
            // house owns its own ally bits — and the game-over scan requires BOTH
            // houses of a pair to name the other, so a one-way alliance must not end
            // the match.
            let all_allied = alive.iter().all(|a| {
                alive.iter().all(|b| {
                    a == b
                        || crate::map::houses::are_houses_mutually_allied(
                            &self.house_alliances,
                            self.interner.resolve(*a),
                            self.interner.resolve(*b),
                        )
                })
            });

            if all_allied {
                for &owner in &alive {
                    if let Some(h) = self.houses.get_mut(&owner)
                        && h.flag_to_win(outcome_tick, savour_frames)
                    {
                        self.sound_events.push(SimSoundEvent::MatchOutcome {
                            owner,
                            kind: crate::sim::house_state::HouseOutcomeKind::Victory,
                        });
                    }
                }
            }
        }

        // HouseClass::Update @ 0x004F8440 advances the accepted result timer
        // in the house rung. The expiry frame is terminal and therefore skips
        // the wrapping frame commit below, matching Main_Tick's early return.
        for house in self.houses.values_mut() {
            house.advance_outcome_savour(outcome_tick);
        }
    }

    /// The defeat gate's test (`0x004F8E92..0x004F8F77`): a house neither
    /// defeated nor `MultiplayPassive=` whose counts say it holds nothing.
    fn house_holds_nothing(
        &self,
        owner: InternedId,
        base_units: &[Option<InternedId>; 3],
        build_refinery_2: Option<InternedId>,
    ) -> bool {
        let Some(house) = self.houses.get(&owner) else {
            return false;
        };
        if house.is_defeated || house.multiplay_passive {
            return false;
        }
        if self.session.game_options.short_game {
            !house.tracking.short_game_alive(base_units)
        } else {
            !house.tracking.normal_game_alive(build_refinery_2)
        }
    }

    /// `HouseClass::MPlayer_Defeated @ 0x004FC0B0`, the represented part: the
    /// Defeated flag, the announcement (`0x004FC30F..0x004FC3BC`, any
    /// non-passive house; the app decides local or other), the map-clear byte
    /// `+0x241` (written at `0x004FC328` for another player's house, through
    /// the whole-map reveal at `0x00577F48` for the local player's) and the
    /// loss.
    fn mplayer_defeated(&mut self, owner: InternedId, outcome_tick: u64, savour_frames: u64) {
        self.sound_events
            .push(SimSoundEvent::PlayerDefeated { house: owner });
        let accepted = self.houses.get_mut(&owner).is_some_and(|house| {
            house.is_defeated = true;
            house.map_is_clear = true;
            house.flag_to_lose(outcome_tick, savour_frames)
        });
        if accepted {
            self.sound_events.push(SimSoundEvent::MatchOutcome {
                owner,
                kind: crate::sim::house_state::HouseOutcomeKind::Defeat,
            });
        }
    }

    /// `HouseClass::Blowup_All @ 0x004FC6D0` (see the module doc).
    pub(crate) fn house_blowup_all(
        &mut self,
        house: InternedId,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
    ) {
        let c4 = self.interner.intern(&rules.bridge_warheads.c4_name);
        // The array is re-read each step (items `0x004FC6EC`, count
        // `0x004FC771`): an object added during the sweep is visited too,
        // after every one that preceded it.
        let mut visited_below = 0u64;
        loop {
            let mut order: Vec<u64> = self
                .substrate
                .entities
                .values()
                .filter(|entity| entity.stable_id() >= visited_below)
                .map(|entity| entity.stable_id())
                .collect();
            if order.is_empty() {
                break;
            }
            order.sort_unstable();
            visited_below = order[order.len() - 1] + 1;
            for id in order {
                if !self.blown_up_with(id, house, rules) {
                    continue;
                }
                // 0x004FC742: the chain warping it lets go first.
                if let Some(head) = self
                    .substrate
                    .entities
                    .get(id)
                    .and_then(|entity| entity.temporal.chain_head())
                {
                    self.temporal_release_chain_no_idle(head, rules);
                }
                let Some(health) = self
                    .substrate
                    .entities
                    .get(id)
                    .map(|entity| entity.health.current)
                else {
                    continue;
                };
                let event = EntityDamageEvent::direct_receiver(
                    id,
                    health,
                    0,
                    RAD_NO_ATTACKER,
                    None,
                    c4,
                    ReceiverCallFlags {
                        ignore_defenses: true,
                        arg6: true,
                    },
                );
                #[cfg(test)]
                BLOWUP_TRACE.with(|trace| trace.borrow_mut().push(event));
                self.commit_direct_damage_receiver(rules, registry, event);
            }
        }
    }

    /// Blowup_All's predicate (`0x004FC6F1..0x004FC731`) for one Techno:
    /// its original owner (`TechnoClass::GetOriginalOwner @ 0x0070F820`) is
    /// `house`, and either it still belongs to `house` or no Civilian-side
    /// house takes it over from the house controlling it.
    fn blown_up_with(&mut self, id: u64, house: InternedId, rules: &RuleSet) -> bool {
        let Some(entity) = self.substrate.entities.get(id) else {
            return false;
        };
        let owner = entity.owner();
        let Some(controller) = entity.mind_control.controller() else {
            return owner == house;
        };
        // CaptureManagerClass::GetOriginalOwner @ 0x004722F0 (null without a
        // node).
        let original = self
            .substrate
            .entities
            .get(controller)
            .and_then(|controller| controller.capture_manager.as_ref())
            .and_then(|manager| manager.original_owner(id));
        if original == Some(owner) {
            return owner == house;
        }
        if original != Some(house) {
            return false;
        }
        !self.set_original_owner_to_civilian(controller, id, rules)
    }

    /// The first house in HouseClass::Array order whose side is `Civilian`
    /// (`SideClass::FindIndex("Civilian") @ 0x006A46D0` against each
    /// house type's side), as `SetOriginalOwnerToCivilian` and the slave
    /// release (`0x006B0B0B..0x006B0B38`) look it up.
    pub(crate) fn civilian_side_house(&self, rules: &RuleSet) -> Option<InternedId> {
        let civilian_side = rules.side_index("Civilian")?;
        self.session.house_order.iter().copied().find(|owner| {
            self.houses
                .get(owner)
                .is_some_and(|house| house.side_index == civilian_side.0)
        })
    }

    /// `CaptureManagerClass::SetOriginalOwnerToCivilian @ 0x00472330`: the
    /// first house in HouseClass::Array order whose side is `Civilian`
    /// (`0x006A46D0`) becomes the victim's original owner in every node of
    /// the controller's manager. False when no such house exists.
    fn set_original_owner_to_civilian(
        &mut self,
        controller: u64,
        victim: u64,
        rules: &RuleSet,
    ) -> bool {
        let Some(civilian) = self.civilian_side_house(rules) else {
            return false;
        };
        if let Some(manager) = self
            .substrate
            .entities
            .get_mut(controller)
            .and_then(|controller| controller.capture_manager.as_mut())
        {
            manager.set_original_owner(victim, civilian);
        }
        true
    }
}

#[cfg(test)]
thread_local! {
    /// Blowup_All's receiver calls, in order, for the native comparison.
    static BLOWUP_TRACE: std::cell::RefCell<Vec<EntityDamageEvent>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
fn take_blowup_trace() -> Vec<EntityDamageEvent> {
    BLOWUP_TRACE.with(|trace| std::mem::take(&mut *trace.borrow_mut()))
}

#[cfg(test)]
#[path = "house_defeat_tests.rs"]
mod tests;
