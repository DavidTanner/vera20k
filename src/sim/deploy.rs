//! Infantry deployment actions and the separate Unit deployment countdown.
//!
//! Infantry's Doing and shared Techno Stage own its sequence. The completion
//! receiver applies the independent crush byte and passive-scan timer effects,
//! including when its requested next action refuses. Unit deployment retains
//! its existing controller; it does not advance an Infantry clock.

use crate::map::entities::EntityCategory;
use crate::rules::ruleset::RuleSet;
use crate::sim::combat::{TargetKind, combat_weapon};
use crate::sim::entity_store::EntityStore;
use crate::sim::mission::{MissionId, MissionType};
use crate::sim::world::{SimSoundEvent, Simulation};

/// Existing Unit deployment control. Infantry uses Doing27..31 instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DeployPhase {
    Deploying { ticks_remaining: u16 },
    Deployed,
    Undeploying { ticks_remaining: u16 },
}

/// Advance only the existing Unit deployment countdown. The native Infantry
/// sequencer520AE0 observes the object's shared Stage during its own AI visit.
pub fn tick_deploy_state(entities: &mut EntityStore) {
    for id in entities.keys_sorted() {
        let Some(entity) = entities.get_mut_if(id, |entity| {
            entity.category == EntityCategory::Unit
                && matches!(
                    entity.deploy_state,
                    Some(DeployPhase::Deploying { .. } | DeployPhase::Undeploying { .. })
                )
                && !entity.ai_frozen()
        }) else {
            continue;
        };
        match entity.deploy_state {
            Some(DeployPhase::Deploying { ticks_remaining }) if ticks_remaining > 1 => {
                entity.deploy_state = Some(DeployPhase::Deploying {
                    ticks_remaining: ticks_remaining - 1,
                });
            }
            Some(DeployPhase::Deploying { .. }) => {
                entity.deploy_state = Some(DeployPhase::Deployed);
            }
            Some(DeployPhase::Undeploying { ticks_remaining }) if ticks_remaining > 1 => {
                entity.deploy_state = Some(DeployPhase::Undeploying {
                    ticks_remaining: ticks_remaining - 1,
                });
            }
            Some(DeployPhase::Undeploying { .. }) => {
                entity.deploy_state = None;
                if let Some(locomotor) = entity.locomotor.as_mut() {
                    locomotor.power_on();
                }
            }
            Some(DeployPhase::Deployed) | None => {}
        }
    }
}

impl Simulation {
    /// Do_Action51D939..51D9CF requests DeploySound/UndeploySound after
    /// admission and before Doing51D9D2 and the Stage restart. The action owner
    /// calls this receiver only for an admitted, changed action.
    pub(crate) fn emit_infantry_deploy_action_sound(
        &mut self,
        id: u64,
        requested: i32,
        rules: &RuleSet,
    ) -> Result<(), String> {
        if !matches!(requested, 27 | 31) {
            return Ok(());
        }
        let actor = self
            .substrate
            .entities
            .get(id)
            .ok_or("deployment sound receiver retired")?;
        let object = self
            .object_type(actor.type_ref(), rules)
            .ok_or("deployment sound requires the Infantry type")?;
        let sound = if requested == 27 {
            object.deploy_sound.as_deref()
        } else {
            object.undeploy_sound.as_deref()
        };
        let Some(sound) = sound else {
            return Ok(());
        };
        let (rx, ry) = (actor.position.rx, actor.position.ry);
        let sound_id = self.interner.intern(sound);
        self.sound_events.push(if requested == 27 {
            SimSoundEvent::EntityDeployed {
                deploy_sound_id: sound_id,
                rx,
                ry,
            }
        } else {
            SimSoundEvent::EntityUndeployed {
                undeploy_sound_id: sound_id,
                rx,
                ry,
            }
        });
        Ok(())
    }

    /// Infantry vtable+54C = Stop callback521B40. Clear6E4 BEFORE calling
    /// unforced Do_Action27: a zero-health action may re-enter Stop_Driver.
    pub(crate) fn infantry_pending_deploy_stop_callback(
        &mut self,
        id: u64,
        rules: Option<&RuleSet>,
    ) -> Result<(), String> {
        let actor = self
            .substrate
            .entities
            .get(id)
            .ok_or("pending deployment Stop receiver retired")?;
        if !actor
            .mission_leaf
            .as_infantry()
            .is_some_and(|leaf| leaf.pending_deploy() != 0)
        {
            return Ok(());
        }
        let rules = rules.ok_or("pending deployment Stop callback requires rules")?;
        self.substrate
            .entities
            .get_mut(id)
            .expect("same pending deployment receiver")
            .mission_leaf
            .take_infantry_pending_deploy();
        self.infantry_do_action(id, 27, false, rules)?;
        Ok(())
    }

    /// Admitted completion arms of DoType_Sequencer520AE0. Its owner tests
    /// signed Stage/count before entering here. The suffix is independent of
    /// Do_Action's return:27→28 then2A4/timer70F770;31→0 then2A4 only.
    /// Original controls: tools/spatial_oracle/infantry_deploy_action.json.
    pub(crate) fn infantry_deploy_completion(
        &mut self,
        id: u64,
        completed_doing: i32,
        rules: &RuleSet,
    ) -> Result<(), String> {
        let next = match completed_doing {
            27 => 28,
            31 => 0,
            other => return Err(format!("deployment completion received Doing{other}")),
        };
        self.infantry_do_action(id, next, true, rules)?;
        //520B3B/520B9A re-read the type AFTER the concrete action receiver.
        let actor = self
            .substrate
            .entities
            .get(id)
            .ok_or("deployment completion receiver retired")?;
        let crushable = self
            .object_type(actor.type_ref(), rules)
            .ok_or("deployment completion requires the Infantry type")?
            .deployed_crushable;
        if !crushable {
            self.substrate
                .entities
                .get_mut(id)
                .expect("same deployment completion receiver")
                .set_infantry_deploy_crush_immunity(u8::from(completed_doing == 27));
        }
        if completed_doing == 27 {
            self.shorten_passive_scan_timer(id);
        }
        Ok(())
    }

    /// Original Infantry Mission_Unload51F6E0. Action admission does not gate
    /// the subsequent weapon, direct Mission::Assign(Guard), or NULL class
    /// destination effects. FootUnload4DA2B0→Mission5B2EF0 returns450.
    pub(crate) fn infantry_mission_unload(
        &mut self,
        id: u64,
        rules: &RuleSet,
    ) -> Result<i32, String> {
        let actor = self
            .substrate
            .entities
            .get(id)
            .ok_or("Infantry Unload receiver retired")?;
        let object = self
            .object_type(actor.type_ref(), rules)
            .ok_or("Infantry Unload requires the type")?;
        let doing = actor
            .mission_leaf
            .as_infantry()
            .ok_or("Infantry Unload requires Infantry Doing")?
            .doing();
        if !object.deployer {
            return Ok(450);
        }
        let mut delay = -1;
        if (27..=30).contains(&doing) {
            if object.undeploy_delay <= -1 {
                self.infantry_do_action(id, 31, true, rules)?;
            }
        } else {
            self.infantry_do_action(id, 27, true, rules)?;
            //70E120 uses the SprayAttack slot70DD70, not GetCurrentWeapon
            //70E1A0. The existing GetWeapon owner resolves the live tier.
            let actor = self.substrate.entities.get(id).expect("same Unload actor");
            let slot = if object.spray_attack { 0 } else { 1 };
            let area_fire = combat_weapon::weapon_for_index(object, actor.veterancy(), slot)
                .and_then(|(weapon, _)| rules.weapon(weapon))
                .is_some_and(|weapon| weapon.area_fire);
            if area_fire {
                //5F3E50→410A40 compares native literal82557C "DESO".
                if object.id.eq_ignore_ascii_case("DESO") {
                    delay = rules
                        .animation_sequence(&object.id)
                        .and_then(|set| set.infantry_action(27))
                        .ok_or("DESO Unload requires its native Deploy sequence record")?
                        .frames_per_facing
                        .wrapping_add(1);
                } else {
                    let coord = self.foot_navigation_coordinate(id)?;
                    let requested = ((coord.x / 256) as i16, (coord.y / 256) as i16);
                    let terrain = self
                        .resolved_terrain
                        .as_ref()
                        .ok_or("AreaFire Unload requires map cells")?;
                    let cell = terrain.native_cell_identity(requested);
                    let at = terrain.native_cell_coord(cell);
                    self.assign_target_represented(
                        id,
                        Some(TargetKind::Cell(at.0 as u16, at.1 as u16)),
                        Some(rules),
                    )
                    .map_err(|cause| format!("AreaFire Unload target: {cause}"))?;
                }
            }
            if object.undeploy_delay > -1 {
                delay = object.undeploy_delay;
            }
        }
        self.mission_assign_exact(
            id,
            MissionId::from_known(MissionType::Guard),
            self.session.binary_frame,
        )
        .map_err(|cause| format!("Infantry Unload Guard: {cause}"))?;
        self.assign_destination_represented(id, None, Some(rules), None)
            .map_err(|cause| format!("Infantry Unload destination: {cause}"))?;
        Ok(if delay > -1 { delay } else { 450 })
    }
}
