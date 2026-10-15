//! Which entities a pointer expiry reaches.
//!
//! `DispatchPointerExpiredCleanup @ 0x007258D0` calls every object's
//! PointerExpired, and most entities hold nothing an expiry changes, so the
//! broadcast passes over them ([`Simulation::entity_expiry_holds`]). This
//! index names, for an expiring id, the entities that hold something, instead
//! of asking every entity at every expiry. It is derived from the entities
//! alone and re-reads an entity whenever the store has handed it out mutably
//! (its touch log), so it answers as asking every entity would. Never saved,
//! never hashed; a fresh, cloned or restored store makes it re-read every
//! entity.

use std::collections::BTreeSet;

use super::Simulation;
use crate::sim::entity_store::{EntityStore, TouchReader};
use crate::sim::game_entity::GameEntity;
use crate::sim::touch_log::Touched;
use crate::util::lookup_hash::LookupMap;

/// The fewest re-read entities a refresh hands to the worker pool.
const PARALLEL_FROM: usize = 256;

/// What one entity holds that an expiry can change.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Holds {
    /// The ids its compared fields name, ascending, without repeats.
    names: Vec<u64>,
    /// It holds an object a forward reaches whatever expires.
    every: bool,
}

impl Holds {
    /// What `entity` holds, or `None` when an expiry passes it over.
    fn of(entity: &GameEntity) -> Option<Self> {
        let mut names = Vec::new();
        let every = Simulation::entity_expiry_holds(entity, |id| names.push(id));
        names.sort_unstable();
        names.dedup();
        (every || !names.is_empty()).then_some(Self { names, every })
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ExpiryListeners {
    /// What each entity holding anything held when last read. Only looked
    /// up; the one walk over it sorts what it finds.
    holds: LookupMap<u64, Holds>,
    /// (named id, entity naming it): an id's namers are one ascending range,
    /// and an entity that stops naming it leaves in one step, however many
    /// others name it too.
    named_by: BTreeSet<(u64, u64)>,
    /// The entities an expiry of anything reaches.
    every: BTreeSet<u64>,
    /// Whether every entity has been read.
    built: bool,
}

impl ExpiryListeners {
    /// The entities `expired`'s expiry reaches, ascending: itself while
    /// stored, those naming it, and those any expiry reaches. The entities
    /// that [`Simulation::entity_expiry_listener_acts`] selects.
    pub(crate) fn reached_by(&mut self, entities: &mut EntityStore, expired: u64) -> Vec<u64> {
        self.refresh(entities);
        let mut reached: Vec<u64> = self.every.iter().copied().collect();
        reached.extend(
            self.named_by
                .range((expired, 0)..=(expired, u64::MAX))
                .map(|&(_, listener)| listener),
        );
        if entities.contains(expired) {
            reached.push(expired);
        }
        reached.sort_unstable();
        reached.dedup();
        reached
    }

    /// Re-read every entity handed out mutably since the last refresh. When
    /// the log cannot say which, or names as many ids as the store holds (its
    /// ids repeat, so sorting them would cost more), re-read every entity and
    /// drop what the removed ones held.
    fn refresh(&mut self, entities: &mut EntityStore) {
        let ids = match entities.take_touched(TouchReader::ExpiryListeners) {
            Touched::Ids(mut ids) if self.built && ids.len() < entities.len() => {
                ids.sort_unstable();
                ids.dedup();
                ids
            }
            _ => {
                self.built = true;
                let mut gone: Vec<u64> = self
                    .holds
                    .keys()
                    .copied()
                    .filter(|&id| !entities.contains(id))
                    .collect();
                gone.sort_unstable();
                for id in gone {
                    self.replace(id, None);
                }
                entities.keys_sorted()
            }
        };
        let entities = &*entities;
        let read = |&id: &u64| entities.get(id).and_then(Holds::of);
        // Handing a few ids to the worker pool costs more than reading them.
        let now: Vec<_> = if ids.len() < PARALLEL_FROM {
            ids.iter().map(read).collect()
        } else {
            crate::sim::parallel::map(&ids, read)
        };
        for (id, now) in ids.into_iter().zip(now) {
            self.replace(id, now);
        }
    }

    fn replace(&mut self, id: u64, now: Option<Holds>) {
        if self.holds.get(&id) == now.as_ref() {
            return;
        }
        if let Some(old) = self.holds.remove(&id) {
            for &name in &old.names {
                self.named_by.remove(&(name, id));
            }
            if old.every {
                self.every.remove(&id);
            }
        }
        if let Some(now) = now {
            self.named_by
                .extend(now.names.iter().map(|&name| (name, id)));
            if now.every {
                self.every.insert(id);
            }
            self.holds.insert(id, now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::capture_manager::CaptureManagerState;
    use crate::sim::combat::TargetKind;

    /// Reach `expired` through the index and check it against asking every
    /// entity with the same enumeration.
    fn reach(index: &mut ExpiryListeners, entities: &mut EntityStore, expired: u64) -> Vec<u64> {
        let reached = index.reached_by(entities, expired);
        assert_eq!(
            reached,
            entities.par_select(|id, entity| {
                Simulation::entity_expiry_listener_acts(entity, id, expired)
            }),
            "index and entities disagree on {expired}"
        );
        reached
    }

    fn attacker(id: u64, target: u64) -> GameEntity {
        let mut entity = GameEntity::test_default(id, "MTNK", "Americans", 10, 10);
        entity
            .attack_target
            .set_for_test(Some(TargetKind::Entity(target)));
        entity
    }

    #[test]
    fn the_index_reaches_what_asking_every_entity_reaches() {
        let mut entities = EntityStore::new();
        entities.insert(attacker(1, 3));
        let mut controller = GameEntity::test_default(2, "YURI", "Russians", 12, 12);
        controller.capture_manager =
            Some(CaptureManagerState::with_victims_for_test(3, false, &[]));
        entities.insert(controller);
        entities.insert(GameEntity::test_default(3, "HTNK", "Russians", 14, 14));
        let mut docked = attacker(4, 3);
        docked.mark_live_contact_with(3);
        entities.insert(docked);
        let mut index = ExpiryListeners::default();

        // Itself, the entities naming it once or twice, and the one that any
        // expiry reaches.
        assert_eq!(reach(&mut index, &mut entities, 3), [1, 2, 3, 4]);
        assert_eq!(reach(&mut index, &mut entities, 9), [2]);
        // A retarget is read back on the next expiry.
        entities
            .get_mut(1)
            .unwrap()
            .attack_target
            .set_for_test(Some(TargetKind::Entity(4)));
        assert_eq!(reach(&mut index, &mut entities, 3), [2, 3, 4]);
        assert_eq!(reach(&mut index, &mut entities, 4), [1, 2, 4]);
        // An entity inserted after the index was built.
        entities.insert(attacker(5, 3));
        assert_eq!(reach(&mut index, &mut entities, 3), [2, 3, 4, 5]);
        // A removed entity reaches nothing and is no listener itself.
        entities.remove(4);
        assert_eq!(reach(&mut index, &mut entities, 4), [1, 2]);
        assert_eq!(reach(&mut index, &mut entities, 3), [2, 3, 5]);
        // A walk over every entity makes it re-read them all.
        for entity in entities.values_mut() {
            entity.capture_manager = None;
        }
        assert_eq!(reach(&mut index, &mut entities, 3), [3, 5]);
        assert_eq!(reach(&mut index, &mut entities, 4), [1]);
    }

    #[test]
    fn many_entities_naming_one_id_are_reread_on_the_worker_pool() {
        let mut entities = EntityStore::new();
        for id in 1..=600 {
            entities.insert(attacker(id, 1000 + id % 3));
        }
        let mut index = ExpiryListeners::default();
        assert_eq!(reach(&mut index, &mut entities, 1000).len(), 200);
        // More hand-outs than stored entities: every entity is re-read, and
        // the removed ones drop out.
        for id in 1..=600 {
            if id % 2 == 0 {
                entities
                    .get_mut(id)
                    .unwrap()
                    .attack_target
                    .set_for_test(Some(TargetKind::Entity(1000)));
            }
            entities.get_mut(id).unwrap();
        }
        for id in (5..=600).step_by(5) {
            entities.remove(id);
        }
        for expired in [1000, 1001, 1002, 5] {
            reach(&mut index, &mut entities, expired);
        }
        // Fewer than the store holds, but enough for the worker pool.
        for id in 1..=400 {
            if let Some(entity) = entities.get_mut(id) {
                entity
                    .attack_target
                    .set_for_test(Some(TargetKind::Entity(1001)));
            }
        }
        for expired in [1000, 1001, 1002] {
            reach(&mut index, &mut entities, expired);
        }
    }
}
