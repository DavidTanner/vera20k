//! Complete mutations of factory-held Techno identity and its accounting.
//!
//! The registry owns queue/charge kernels. This world-facing owner completes
//! their constructor, disposal, ready projection and successor work before an
//! operation returns. Native StartProduction/AbandonProduction/StartNextQueued:
//! 0x004C9C70 / 0x004C9FF0 / 0x004CA5A0; see FACTORY_CREDIT_SYSTEM_GHIDRA_REPORT.md.
//! Delivery selection and placement keep their existing phase positions and
//! call settlement only after their successful world effects have committed.

use super::CancelOutcome;
use super::factory::{
    AbandonedObject, EnqueueOutcome, FactoryHolder, time_to_build, time_to_build_inputs,
};
use super::production_tech::{
    build_option_for_owner, production_category_for_object, supports_live_production,
};
use super::production_types::{BuildDisabledReason, ProductionCategory};
use crate::rules::object_type::ObjectCategory;
use crate::rules::ruleset::RuleSet;
use crate::sim::ai_unit_choice::UnitChoiceKind;
use crate::sim::intern::InternedId;
use crate::sim::world::{SimSoundEvent, Simulation};

/// The PRODUCE event for `type_id`: `HouseClass::Begin_Production @ 0x004FA350`.
///
/// The build needs a factory that may build it: VERA's build option, for
/// gamemd's `FindFactory(0,1,1)` (`0x004FA438`), whose `CanBuild(type, 1, 1)`
/// lets a type at its build limit through while one of the house's factories
/// holds it (`0x004F8348`). A stopped active object of the same type takes the
/// same-type branch (`0x004FA5A8..0x004FA5C4`): a held build resumes, and a
/// finished one stays (the build start refuses stage 54, `0x004C9ECD`).
/// Otherwise `FactoryClass::StartProduction @ 0x004C9C70` starts or queues
/// the build. Nothing on this path checks money: a short wallet stalls the
/// steps instead. A queue already holding `[General] MaximumQueuedObjects=`
/// builds refuses the append (`0x004C9CDE`) and scolds the local player.
///
/// Residual: an append refused at the build limit (`0x004C9CEA`) scolds as
/// well; VERA refuses it before StartProduction, silently. Trigger: a second
/// click on a limited type whose first PRODUCE has not executed yet. Rare,
/// sound only.
///
/// Residual (building path): a building never queues in gamemd.
/// Begin_Production refuses a building PRODUCE while the building factory runs
/// (`0x004FA52E..0x004FA550`), and StartProduction abandons a held or finished
/// building before starting a different one (`0x004C9C87..0x004C9C98`); VERA
/// appends it. Trigger: a second building PRODUCE before the first executes
/// (the sidebar refuses a busy structure strip, and VERA's AI asks only while
/// no building is queued). Effect: VERA builds both in turn. Rare.
pub fn enqueue_by_type(sim: &mut Simulation, rules: &RuleSet, owner: &str, type_id: &str) -> bool {
    let Some(option) = build_option_for_owner(sim, rules, owner, type_id) else {
        return false;
    };
    let Some(obj) = rules.object(type_id) else {
        return false;
    };
    if !supports_live_production(obj) {
        return false;
    }
    let category = production_category_for_object(obj);
    let owner_id = sim.interner.intern(owner);
    let type_interned = sim.interner.intern(type_id);
    if let Some((active, held, finished)) =
        sim.production.factories.active_object(owner_id, category)
        && active == type_interned
        && (held || finished)
    {
        let may_resume = matches!(
            option.reason,
            None | Some(BuildDisabledReason::AtBuildLimit)
        );
        if !held || !may_resume {
            return false;
        }
        let time_to_build =
            time_to_build(&time_to_build_inputs(sim, rules, owner_id, category, obj));
        let frame = sim.session.binary_frame;
        sim.production
            .factories
            .resume(owner_id, category, time_to_build, frame);
        return true;
    }
    if !option.enabled {
        return false;
    }
    let enqueue_order = sim.production.next_enqueue_order;
    let cost = sim.cost_of(owner_id, obj, rules);
    let outcome = sim.production.factories.enqueue(
        owner_id,
        category,
        type_interned,
        enqueue_order,
        cost,
        rules.general.maximum_queued_objects,
    );
    if outcome == EnqueueOutcome::QueueFull {
        sim.sound_events
            .push(SimSoundEvent::ProductionRefused { owner: owner_id });
        return false;
    }
    sim.production.next_enqueue_order = enqueue_order.saturating_add(1);
    if outcome == EnqueueOutcome::Started {
        start_active_production(
            sim,
            rules,
            FactoryHolder::House(owner_id, category),
            type_interned,
        )
        .expect("validated StartProduction type must construct one Techno");
    }
    true
}

/// The SUSPEND event: `HouseClass::Suspend_Production @ 0x004FA910` puts the
/// category's running build on hold.
pub fn suspend_production(sim: &mut Simulation, owner: &str, category: ProductionCategory) -> bool {
    let Some(owner_id) = sim.interner.get(owner) else {
        return false;
    };
    let frame = sim.session.binary_frame;
    sim.production.factories.suspend(owner_id, category, frame)
}

/// Start the build an active factory head holds. `HouseClass::Begin_Production
/// @ 0x004FA350` runs `FactoryClass::StartProduction @ 0x004C9C70`, which calls
/// `type->CreateInstance(owner)` and stores the result at `Factory+0x58`, then
/// the build start `0x004C9EA0` (Ghidra label `FactoryClass__SetRate`) at
/// `0x004FA628`, which arms the step rate and timer from that object's
/// `Time_To_Build`. Queued tail entries start only when
/// `FactoryClass::StartNextQueued @ 0x004CA5A0` promotes them, which runs the
/// same Begin_Production (`0x004CA60A`).
///
/// Begin_Production's network headstart (`0x004FA631..0x004FA68F`) never runs:
/// it needs the factory unsuspended just before the start (read at
/// `0x004FA622`), and every path there has just suspended it (StartProduction's
/// create path, `0x004C9D72`) or resumed a suspended one (`0x004FA5A8..0x004FA5C6`).
/// A queue append (`0x004C9D22..0x004C9D2E`) returns at `0x004FA612` first, and
/// a promotion's StartProduction never appends (`0x004C9CC9..0x004C9CCF`).
///
/// A computer's building factory runs the same StartProduction and build
/// start (`BuildingClass::Factory_AI`, `0x0045039A` and `0x004503C5`).
/// StartProduction also marks a computer house's building
/// (`+0x6CA = 1`, `0x004C9DC5`), the `[Structures]` AI Rebuildable byte
/// (`0x0044FB61`), which only the map writer and the building's CRC read;
/// VERA does not keep it.
pub(super) fn start_active_production(
    sim: &mut Simulation,
    rules: &RuleSet,
    holder: FactoryHolder,
    type_id: InternedId,
) -> Option<u64> {
    let (owner_id, category) = sim
        .production
        .factories
        .factory(holder)
        .map(|factory| (factory.owner, factory.category))?;
    let obj = sim.object_type(type_id, rules)?;
    let owner = sim.interner.resolve(owner_id).to_string();
    let type_name = sim.interner.resolve(type_id).to_string();
    // A factory-held object is still in limbo and has no cell authority. Zero
    // is only inert storage here; the result-bearing Unlimbo later installs the
    // selected delivery/placement coordinate on this same stable identity.
    let stable_id = sim.construct_object_limbo_at_height(&type_name, &owner, 0, 0, 0, 0, rules)?;
    let linked = sim
        .production
        .factories
        .link_active_entity(holder, stable_id);
    if linked != Some(stable_id) {
        let _ = sim.discard_constructed_limbo(stable_id, Some(rules));
        return None;
    }
    let time_to_build = time_to_build(&time_to_build_inputs(sim, rules, owner_id, category, obj));
    let frame = sim.session.binary_frame;
    sim.production
        .factories
        .start_rate(holder, time_to_build, frame);
    Some(stable_id)
}
/// Finish `FactoryClass::AbandonProduction @ 0x004C9FF0` for an object the registry
/// let go: refund it ([`refund_abandoned`]); a house no human controls then
/// forgets its choice of that kind (`0x004CA082..0x004CA0DD`: the building
/// choice of `sim::ai_base_building`, or a choice of `sim::ai_unit_choice`);
/// then the held limbo object is destroyed without rewinding RNG
/// (`0x004CA0E0`).
pub(super) fn settle_abandoned(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner_id: InternedId,
    abandoned: AbandonedObject,
) {
    refund_abandoned(sim, rules, owner_id, abandoned.type_id, abandoned.balance);
    let game_mode_nonzero = sim.session.game_mode_nonzero;
    let category = sim
        .object_type(abandoned.type_id, rules)
        .map(|object| object.category);
    if let Some(category) = category
        && let Some(house) = sim.houses.get_mut(&owner_id)
        && !house.is_controlled_by_human(game_mode_nonzero)
    {
        match UnitChoiceKind::of(category) {
            Some(kind) => house.ai_unit_choices.clear(kind),
            None => house.ai_production.clear_building_choice(),
        }
    }
    if let Some(entity_id) = abandoned.entity_id {
        // Original4CA0E3..4CA109 brackets the scalar destructor, including
        // Building43BD67 pointer expiry. Its listener timers cannot draw under
        // A8E7AC; preserve the caller's bracket through the shared destructor.
        let discarded = sim.with_object_placement_scope(|sim| {
            sim.discard_constructed_limbo(entity_id, Some(rules))
        });
        debug_assert!(
            discarded,
            "AbandonProduction destroys the held limbo object"
        );
    }
}

/// The AbandonProduction refund: the object's Cost_Of for its owner now, less the
/// Balance it still owed (`0x004CA029..0x004CA043`), through `HouseClass::Add_Credits
/// @ 0x004F9950` (`0x004CA046`). A FactoryPlant gained or lost during the build
/// changes that Cost_Of, so the refund can differ from what was paid, negative
/// included.
fn refund_abandoned(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner_id: InternedId,
    type_id: InternedId,
    balance: i32,
) {
    let Some(object) = sim.object_type(type_id, rules) else {
        return;
    };
    let refund = sim.cost_of(owner_id, object, rules).wrapping_sub(balance);
    crate::sim::credit_income::add_credits(sim, owner_id, refund);
}

/// The ABANDON / ABANDON_ALL events for `type_id` (a right click, with Shift
/// for all): `HouseClass::Abandon_Production @ 0x004FAA10` through the
/// registry's `cancel_one`. An abandoned active object is refunded and
/// destroyed ([`settle_abandoned`]), a finished building stops waiting for
/// placement, and the next queued build starts (`0x004FAC96`).
pub fn cancel_by_type_for_owner(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: &str,
    type_id: &str,
    all: bool,
) -> bool {
    let Some(obj) = rules.object(type_id) else {
        return false;
    };
    let category = production_category_for_object(obj);
    let owner_id = sim.interner.intern(owner);
    let type_interned = sim.interner.intern(type_id);
    let outcome = sim
        .production
        .factories
        .cancel_one(owner_id, category, type_interned, all);
    match outcome {
        CancelOutcome::NoMatch => return false,
        CancelOutcome::QueuedRemoved => {}
        CancelOutcome::AbandonedActive { object, finished } => {
            settle_abandoned(sim, rules, owner_id, object);
            if finished {
                remove_ready_entry(sim, owner_id, type_interned);
            }
            advance_after_delivery(sim, rules, owner_id, category);
        }
    }
    sim.production.factories.prune_all_idle();
    true
}

/// Drop one `ready_by_owner` entry of `type_id`: its finished building left
/// the factory.
fn remove_ready_entry(sim: &mut Simulation, owner: InternedId, type_id: InternedId) -> bool {
    let Some(ready_queue) = sim.production.ready_by_owner.get_mut(&owner) else {
        return false;
    };
    let Some(index) = ready_queue.iter().position(|&ready| ready == type_id) else {
        return false;
    };
    ready_queue.remove(index);
    if ready_queue.is_empty() {
        sim.production.ready_by_owner.remove(&owner);
    }
    true
}

/// C7 StartNextQueued once the active object is gone: after a successful delivery, an
/// abandon or a completed-but-undeliverable refund. Clear the active object and promote
/// the next queued entry into the active slot, cost-seeded from `rules` and started at
/// this frame.
///
/// Human mobile release occurs only in the queued PLACE tail
/// (Event4C710B -> House4FB0E0 -> Abandon4FB663 -> StartNext4FAC96),
/// after Strip has left the completed object's identity held for one frame.
fn advance_after_delivery(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner_id: InternedId,
    category: ProductionCategory,
) {
    let next_cost = sim
        .production
        .factories
        .peek_next_queued(owner_id, category)
        .and_then(|t| sim.object_type(t, rules))
        .map_or(0, |object| sim.cost_of(owner_id, object, rules));
    let promoted = sim
        .production
        .factories
        .clear_active_and_advance(owner_id, category, next_cost);
    if let Some(type_id) = promoted {
        start_active_production(
            sim,
            rules,
            FactoryHolder::House(owner_id, category),
            type_id,
        )
        .expect("validated promoted production type must construct one Techno");
    }
}
pub(super) fn active_entity_id(
    sim: &Simulation,
    owner_id: InternedId,
    category: ProductionCategory,
) -> Option<u64> {
    sim.production
        .factories
        .view(owner_id, category)
        .and_then(|view| view.object.and_then(|object| object.entity_id))
        .filter(|&stable_id| sim.substrate.entities.contains(stable_id))
}
fn discard_active_factory_entity(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner_id: InternedId,
    category: ProductionCategory,
) {
    if let Some(stable_id) = active_entity_id(sim, owner_id, category) {
        let discarded = sim.with_object_placement_scope(|sim| {
            sim.discard_constructed_limbo(stable_id, Some(rules))
        });
        debug_assert!(
            discarded,
            "factory-held object must remain in limbo until delivery"
        );
    }
}
fn consume_ready_building(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner_id: InternedId,
    type_id: InternedId,
    category: ProductionCategory,
) -> bool {
    if !remove_ready_entry(sim, owner_id, type_id) {
        return false;
    }
    advance_after_delivery(sim, rules, owner_id, category);
    true
}
/// Publish the completion edge once, without releasing the held object.
pub(super) fn publish_completion(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: InternedId,
    category: ProductionCategory,
) {
    let Some(type_id) = sim
        .production
        .factories
        .view(owner, category)
        .and_then(|view| view.object.map(|object| object.type_id))
    else {
        return;
    };
    if !sim
        .production
        .factories
        .account_completed_object_once(owner, category)
    {
        return;
    }
    if rules
        .object(sim.interner.resolve(type_id))
        .is_some_and(|object| object.category == ObjectCategory::Building)
    {
        sim.production
            .ready_by_owner
            .entry(owner)
            .or_default()
            .push_back(type_id);
        sim.sound_events
            .push(SimSoundEvent::BuildingComplete { owner });
    }
}

/// Delivery effects already committed; retain the revealed identity and start
/// its successor. A refused vehicle delivery must never call this operation.
pub(in crate::sim) fn release_delivered_mobile(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: InternedId,
    category: ProductionCategory,
) {
    if let Some(object) = sim
        .production
        .factories
        .view(owner, category)
        .and_then(|view| view.object)
    {
        record_last_built(sim, rules, owner, object.type_id);
    }
    advance_after_delivery(sim, rules, owner, category);
}

/// `HouseClass::Record_Last_Built @ 0x004FB6B0` for an object of type
/// `type_id` that left its factory: `HouseClass::Place_Production` calls it
/// after a successful exit (`0x004FB4B7`: a player's placed building, a
/// delivered unit) and a computer's building factory after its placement
/// (`BuildingClass::Factory_AI`, `0x004501A4`). The object's kind counter
/// (`+0x55A0`, `+0x55B4`, `+0x55C8` or `+0x55DC`) grows unless its type is
/// `DontScore=` (`+0xC9F`);
/// [`MatchStatistics::built`](crate::sim::house_state::MatchStatistics) is
/// the sum of the four. Its other writes are residuals of
/// `sim::ai_base_building`.
pub(super) fn record_last_built(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: InternedId,
    type_id: InternedId,
) {
    let scores = sim
        .object_type(type_id, rules)
        .is_some_and(|ty| !ty.dont_score);
    if scores && let Some(house) = sim.houses.get_mut(&owner) {
        house.stats.record_built();
    }
}

/// Terminal mobile failure abandons the finished object with the AbandonProduction
/// refund (its whole Cost_Of, as it owes no Balance), destroys the held graph and
/// starts the successor. This is distinct from a retryable refusal.
pub(super) fn refund_failed_delivery(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: InternedId,
    category: ProductionCategory,
) {
    let type_id = sim
        .production
        .factories
        .view(owner, category)
        .and_then(|view| view.object.map(|object| object.type_id));
    if let Some(type_id) = type_id {
        refund_abandoned(sim, rules, owner, type_id, 0);
    }
    discard_active_factory_entity(sim, rules, owner, category);
    advance_after_delivery(sim, rules, owner, category);
}

/// Matching identity captured by a successful placement admission. The caller
/// cannot combine an entity ID with another owner's type or factory category.
pub(super) struct ReadyFactoryObject {
    owner: InternedId,
    category: ProductionCategory,
    type_id: InternedId,
    entity_id: u64,
}

impl ReadyFactoryObject {
    pub(super) fn entity_id(&self) -> u64 {
        self.entity_id
    }

    /// Building Unlimbo/build-up/superweapon effects precede factory release.
    pub(super) fn release_after_placement(self, sim: &mut Simulation, rules: &RuleSet) -> bool {
        consume_ready_building(sim, rules, self.owner, self.type_id, self.category)
    }

    /// Primary and autofill overlays are already stamped when the constructor
    /// identity is consumed. Ready removal and successor construction follow.
    pub(super) fn consume_after_wall_stamp(self, sim: &mut Simulation, rules: &RuleSet) -> bool {
        let _ = sim.discard_constructed_limbo(self.entity_id, Some(rules));
        record_last_built(sim, rules, self.owner, self.type_id);
        consume_ready_building(sim, rules, self.owner, self.type_id, self.category)
    }
}

pub(super) fn ready_object(
    sim: &Simulation,
    owner: InternedId,
    category: ProductionCategory,
    type_id: InternedId,
) -> Option<ReadyFactoryObject> {
    let object = sim.production.factories.view(owner, category)?.object?;
    if object.type_id != type_id {
        return None;
    }
    let entity_id = object
        .entity_id
        .filter(|&id| sim.substrate.entities.contains(id))?;
    Some(ReadyFactoryObject {
        owner,
        category,
        type_id,
        entity_id,
    })
}

/// Fixture-only bridge for tests that deliberately seed a registry kernel.
#[cfg(test)]
pub(in crate::sim) fn construct_active_factory_fixture(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: InternedId,
    category: ProductionCategory,
    type_id: InternedId,
) -> Option<u64> {
    start_active_production(sim, rules, FactoryHolder::House(owner, category), type_id)
}

/// Revalidate before the charge sweep at its existing frame phase. Dispose all
/// abandoned objects before starting any promoted ones.
pub(in crate::sim) fn revalidate_and_step_factories(sim: &mut Simulation, rules: &RuleSet) {
    let mut registry = std::mem::take(&mut sim.production.factories);
    // P6: prereq/factory-loss revalidation BEFORE the charge sweep. Builds whose
    // prerequisites or producing factory were lost are abandoned and refunded
    // + now-unbuildable queued items dropped, so a freshly-abandoned factory is not
    // charged this tick; a promoted build is started at this frame.
    let reval_plan = registry.plan_revalidation(sim, rules);
    let lifecycle = registry.apply_revalidation(&reval_plan);
    for (owner, abandoned) in lifecycle.abandoned {
        settle_abandoned(sim, rules, owner, abandoned);
    }
    // An abandoned finished building no longer waits for placement.
    for (owner, type_id) in lifecycle.abandoned_finished {
        remove_ready_entry(sim, owner, type_id);
    }
    sim.production.factories = registry;
    for (owner, category, type_id) in lifecycle.promoted {
        start_active_production(sim, rules, FactoryHolder::House(owner, category), type_id)
            .expect("validated revalidation promotion must construct one Techno");
    }
    let mut registry = std::mem::take(&mut sim.production.factories);
    registry.step_all(&mut sim.houses, sim.session.binary_frame);
    sim.production.factories = registry;
}

/// House508D88 invokes the existing Factory rate owner after power changes.
pub(in crate::sim) fn refresh_factory_rates_for_house(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: InternedId,
) {
    let mut registry = std::mem::take(&mut sim.production.factories);
    registry.refresh_rates_for_house(sim, rules, owner);
    sim.production.factories = registry;
}

/// A saved relationship that the live factory operations cannot publish.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("factory state for owner {owner}: {reason}")]
pub(crate) struct FactoryRestoreError {
    pub(crate) owner: InternedId,
    pub(crate) reason: &'static str,
}

/// Validate factory relationships before a loaded candidate becomes a match.
///
/// VERA-internal snapshot admission; gamemd equivalent UNCHECKED. These checks
/// follow the synchronous constructor/publication/settlement operations above,
/// not a new queue, refund or gameplay policy. Generic snapshot reference and
/// LogicVector validation precede this operation. Missing Houses and unrelated
/// limbo objects remain supported; manager pointers are not globally reciprocal.
pub(crate) fn validate_restored_factory_state(
    sim: &Simulation,
    rules: &RuleSet,
) -> Result<(), FactoryRestoreError> {
    use crate::map::entities::EntityCategory;
    use std::collections::{BTreeMap, BTreeSet};

    let fail = |owner, reason| FactoryRestoreError { owner, reason };
    let mut ready = BTreeMap::new();
    for (&owner, entries) in &sim.production.ready_by_owner {
        if sim.interner.try_resolve(owner).is_none() {
            return Err(fail(owner, "ready owner is absent from the interner"));
        }
        for &type_id in entries {
            let object = sim
                .interner
                .try_resolve(type_id)
                .and_then(|name| rules.object(name))
                .ok_or_else(|| fail(owner, "ready type is absent from bound rules"))?;
            if object.category != ObjectCategory::Building {
                return Err(fail(owner, "ready entry is not a building"));
            }
            let category = production_category_for_object(object);
            if ready.insert((owner, category), type_id).is_some() {
                return Err(fail(owner, "multiple ready entries claim one factory"));
            }
            let factory = sim
                .production
                .factories
                .view(owner, category)
                .ok_or_else(|| fail(owner, "ready entry has no factory"))?;
            let held = factory
                .object
                .ok_or_else(|| fail(owner, "ready entry has no active object"))?;
            if held.type_id != type_id {
                return Err(fail(owner, "ready type disagrees with active object"));
            }
            if !factory.ready || !held.completion_accounted {
                return Err(fail(owner, "ready entry precedes completion publication"));
            }
        }
    }

    let mut roots = BTreeSet::new();
    for (&holder, factory) in sim.production.factories.keyed_factories() {
        let (owner, category) = (factory.owner, factory.category);
        match holder {
            FactoryHolder::House(key_owner, key_category) => {
                if owner != key_owner || category != key_category {
                    return Err(fail(owner, "registry key disagrees with factory identity"));
                }
            }
            // `BuildingClass::Factory_AI` makes one for its building's owner,
            // holding one object and no queue, and counts no completion.
            FactoryHolder::Building(building) => {
                let held_by_owner = sim.substrate.entities.get(building).is_some_and(|entity| {
                    entity.category == EntityCategory::Structure && entity.owner() == owner
                });
                if !held_by_owner {
                    return Err(fail(
                        owner,
                        "factory's building is absent or another house's",
                    ));
                }
                if factory.object.is_none() || !factory.queue.is_empty() {
                    return Err(fail(owner, "building factory holds no single object"));
                }
                if factory
                    .object
                    .as_ref()
                    .is_some_and(|object| object.completion_accounted)
                {
                    return Err(fail(owner, "building factory accounted a completion"));
                }
            }
        }
        if sim.interner.try_resolve(owner).is_none() {
            return Err(fail(owner, "factory owner is absent from the interner"));
        }
        for queued in &factory.queue {
            let object = sim
                .interner
                .try_resolve(queued.type_id)
                .and_then(|name| rules.object(name))
                .ok_or_else(|| fail(owner, "queued type is absent from bound rules"))?;
            if production_category_for_object(object) != category {
                return Err(fail(owner, "queued type disagrees with factory category"));
            }
        }
        let Some(held) = &factory.object else {
            if !factory.queue.is_empty() {
                return Err(fail(owner, "queued tail has no active object"));
            }
            continue;
        };
        let object = sim
            .interner
            .try_resolve(held.type_id)
            .and_then(|name| rules.object(name))
            .ok_or_else(|| fail(owner, "active type is absent from bound rules"))?;
        if production_category_for_object(object) != category {
            return Err(fail(owner, "active type disagrees with factory category"));
        }
        let entity_id = held
            .entity_id
            .ok_or_else(|| fail(owner, "active object has no constructed identity"))?;
        if !roots.insert(entity_id) {
            return Err(fail(
                owner,
                "constructed identity belongs to multiple factories",
            ));
        }
        let entity = sim.substrate.entities.get(entity_id).ok_or_else(|| {
            fail(
                owner,
                "constructed identity is absent from the entity store",
            )
        })?;
        if entity.owner() != owner || entity.type_ref() != held.type_id {
            return Err(fail(
                owner,
                "constructed identity disagrees with factory owner or type",
            ));
        }
        let expected_category = EntityCategory::from(object.category);
        if entity.category != expected_category {
            return Err(fail(
                owner,
                "constructed identity has the wrong concrete category",
            ));
        }
        if entity.spawn_owner_id.is_some() || entity.slave.owner().is_some() {
            return Err(fail(owner, "factory root is itself a manager child"));
        }
        // Factory admission validates retained identity and membership, without
        // inferring health or death flags. Terminal-state admission separately
        // validates the object's lifecycle handoff.
        if !entity.lifecycle.in_limbo || entity.lifecycle.cell_marked || entity.in_logic_vector {
            return Err(fail(
                owner,
                "factory object is already admitted to the world",
            ));
        }
        if held.completion_accounted && factory.progress < super::PRODUCTION_STEPS {
            return Err(fail(
                owner,
                "completion accounting precedes completed progress",
            ));
        }
        if held.completion_accounted
            && object.category == ObjectCategory::Building
            && ready.get(&(owner, category)) != Some(&held.type_id)
        {
            return Err(fail(owner, "accounted building has no ready entry"));
        }
    }

    // Restrict only factory roots: native manager pointers elsewhere can alias
    // without implying reciprocal ownership, and ordinary limbo is not a root.
    for (_, factory) in sim.production.factories.keyed_factories() {
        let owner = factory.owner;
        let Some(parent) = factory.object.as_ref().and_then(|object| object.entity_id) else {
            continue;
        };
        let entity = sim
            .substrate
            .entities
            .get(parent)
            .expect("validated factory root");
        let spawn_alias = entity.spawn_manager.as_ref().is_some_and(|manager| {
            manager
                .slots
                .iter()
                .filter_map(|slot| slot.spawn)
                .any(|id| roots.contains(&id))
        });
        let slave_alias = entity
            .slave_manager
            .as_ref()
            .is_some_and(|manager| manager.slaves().any(|id| roots.contains(&id)));
        if spawn_alias || slave_alias {
            return Err(fail(owner, "factory root aliases a held constructor child"));
        }
    }
    Ok(())
}
