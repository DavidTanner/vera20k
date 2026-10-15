//! Precomputed `InternedId -> TypeHandle` map for one-hop entity->type resolution.
//!
//! An entity carries its type as an `InternedId` (`type_ref`). Resolving that to a
//! `&ObjectType` the naive way is two hops: `interner.resolve(id)` (an array index)
//! then `RuleSet::object(name)` (an uppercase `String` allocation + hash lookup).
//! This table collapses the second hop to a precomputed array index, so resolution
//! is two array indexes with no allocation and no hashing.
//!
//! Built once at sim init from a `RuleSet` plus the populated `StringInterner`
//! (after `Simulation::intern_rule_type_ids`). It is a derived cache — read-only during
//! ticks, never serialized, rebuilt on load.
//!
//! ## Dependency rules
//! - Part of sim/; depends on rules/ (one-way) and `sim::intern`. NEVER on
//!   render/ui/audio/net.

use std::collections::HashMap;

use crate::rules::object_type::ObjectCategory;
use crate::rules::ruleset::{RuleSet, TypeHandle};
use crate::sim::intern::{InternedId, StringInterner};

/// A TechnoType's native identity as EventClass records carry it: its
/// registry and its index in that registry's vector (`+0xDF8`), plus the
/// naval flag (`Type+0xCCE`) the factory events send beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeTechnoType {
    pub category: ObjectCategory,
    pub array_index: i32,
    pub naval: bool,
}

/// Maps an interned type id to its object handle. Dense by `InternedId` index;
/// `None` means the id was interned but names no object (an orphan `type_ref`,
/// e.g. a registry id listed without a `[section]`).
///
/// It also holds the native vector indices command records name types by,
/// both ways: TechnoTypes per registry, and SuperWeaponTypes by their
/// `[SuperWeaponTypes]` position.
#[derive(Debug, Clone, Default)]
pub struct TypeHandleTable {
    by_interned: Vec<Option<TypeHandle>>,
    native_techno: Vec<Option<NativeTechnoType>>,
    techno_by_native: HashMap<(ObjectCategory, i32), InternedId>,
    super_index: Vec<Option<i32>>,
    super_by_index: Vec<Option<InternedId>>,
    /// Distinct for every build (0 while unbuilt), so a cache of reads made
    /// through the table can tell a rebuilt table from the one it read.
    build_serial: u64,
}

impl TypeHandleTable {
    /// Build from every string currently in the interner. Each id that resolves
    /// to an object (case-insensitively) gets its handle; the rest stay `None`.
    /// Call after `Simulation::intern_rule_type_ids` so every type id is present.
    pub fn build(rules: &RuleSet, interner: &StringInterner) -> Self {
        let mut by_interned = Vec::with_capacity(interner.len());
        let mut native_techno = Vec::with_capacity(interner.len());
        let mut techno_by_native = HashMap::new();
        let mut super_index = Vec::with_capacity(interner.len());
        let mut super_by_index = vec![None; rules.super_weapon_order.len()];
        for idx in 0..interner.len() as u32 {
            let id = InternedId::from_index(idx);
            // idx < interner.len(), so resolve() is in bounds.
            let name = interner.resolve(id);
            let handle = rules.type_handle(name);
            by_interned.push(handle);
            let native = handle.and_then(|handle| {
                let object = rules.object_by_handle(handle);
                Some(NativeTechnoType {
                    category: object.category,
                    array_index: rules.type_array_index(object.category, name)?,
                    naval: object.naval,
                })
            });
            if let Some(native) = native {
                // The first interned spelling of a type keeps the reverse slot.
                techno_by_native
                    .entry((native.category, native.array_index))
                    .or_insert(id);
            }
            native_techno.push(native);
            let index = rules.super_weapon_index(name);
            if let Some(index) = index
                && super_by_index[index].is_none()
            {
                super_by_index[index] = Some(id);
            }
            super_index.push(index.and_then(|index| i32::try_from(index).ok()));
        }
        static BUILDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        Self {
            by_interned,
            native_techno,
            techno_by_native,
            super_index,
            super_by_index,
            build_serial: BUILDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1,
        }
    }

    /// The native registry identity of an interned TechnoType id.
    pub fn native_techno(&self, id: InternedId) -> Option<NativeTechnoType> {
        self.native_techno
            .get(id.index() as usize)
            .copied()
            .flatten()
    }

    /// The interned id of the TechnoType at `array_index` in `category`.
    pub fn techno_at(&self, category: ObjectCategory, array_index: i32) -> Option<InternedId> {
        self.techno_by_native.get(&(category, array_index)).copied()
    }

    /// The `[SuperWeaponTypes]` index of an interned super weapon id.
    pub fn super_weapon_index(&self, id: InternedId) -> Option<i32> {
        self.super_index.get(id.index() as usize).copied().flatten()
    }

    /// The interned id of the super weapon type at `index`.
    pub fn super_weapon_at(&self, index: i32) -> Option<InternedId> {
        self.super_by_index
            .get(usize::try_from(index).ok()?)
            .copied()
            .flatten()
    }

    /// Which build this table is (see `build_serial`).
    pub(crate) fn build_serial(&self) -> u64 {
        self.build_serial
    }

    /// Resolve an interned id to its handle, if it names an object.
    #[inline]
    pub fn handle_for(&self, id: InternedId) -> Option<TypeHandle> {
        self.by_interned.get(id.index() as usize).copied().flatten()
    }

    /// True if no entries were built (e.g. the table has not been resolved yet).
    pub fn is_empty(&self) -> bool {
        self.by_interned.is_empty()
    }

    /// Resolve a type in one precomputed hop, falling back to the name path
    /// only while the table is unbuilt (fixtures that skip resolution).
    #[inline]
    pub fn object<'r>(
        &self,
        interner: &StringInterner,
        id: InternedId,
        rules: &'r RuleSet,
    ) -> Option<&'r crate::rules::object_type::ObjectType> {
        match self.handle_for(id) {
            Some(handle) => Some(rules.object_by_handle(handle)),
            None if self.is_empty() => rules.object(interner.resolve(id)),
            None => None,
        }
    }

    /// Count of interned ids that did NOT resolve to an object (orphans).
    #[cfg(test)]
    pub fn orphan_count(&self) -> usize {
        self.by_interned.iter().filter(|h| h.is_none()).count()
    }
}

/// Sim-owned pre-resolved rule handles (F04): the `[CombatDamage]` bridge
/// warhead identities combat compares per detonation. `RuleSet` keeps only the
/// canonical names (`rules.bridge_warheads`); the interned ids live here,
/// built beside `TypeHandleTable` after interning and rebuilt from rules on
/// load. Derived cache — never serialized, never hashed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedRuleHandles {
    /// Resolved `IonCannonWarhead=` (retail default `IonCannonWH`).
    pub ion_cannon: crate::sim::intern::InternedId,
    /// Resolved `C4Warhead=` (retail default `Super`).
    pub c4: crate::sim::intern::InternedId,
    /// Resolved `CrushWarhead=` (retail default `Crush`).
    pub crush: crate::sim::intern::InternedId,
}

impl ResolvedRuleHandles {
    /// Intern the canonical warhead names and pin their ids. Idempotent for an
    /// interner that already carries the names (init and every load path), so
    /// re-resolution cannot grow or reorder serialized interner state.
    pub fn resolve(
        rules: &crate::rules::ruleset::RuleSet,
        interner: &mut crate::sim::intern::StringInterner,
    ) -> Self {
        Self {
            ion_cannon: interner.intern(&rules.bridge_warheads.ion_cannon_name),
            c4: interner.intern(&rules.bridge_warheads.c4_name),
            crush: interner.intern(&rules.bridge_warheads.crush_name),
        }
    }

    /// Whether an interned warhead is the resolved `CrushWarhead=`.
    pub fn is_crush(self, warhead_id: crate::sim::intern::InternedId) -> bool {
        self.crush == warhead_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::ini_parser::IniFile;

    /// Minimal ruleset with a single infantry object `E1`.
    fn tiny_rules() -> RuleSet {
        let ini = IniFile::from_str("[InfantryTypes]\n0=E1\n\n[E1]\nStrength=100\n");
        RuleSet::from_ini(&ini).expect("minimal ruleset should parse")
    }

    #[test]
    fn handle_for_resolves_case_insensitively() {
        let rules = tiny_rules();
        let mut interner = StringInterner::new();
        // Intern the lowercased reference; the section header is `E1`.
        let id = interner.intern("e1");
        let table = TypeHandleTable::build(&rules, &interner);
        assert_eq!(table.handle_for(id), rules.type_handle("E1"));
        assert!(table.handle_for(id).is_some());
        assert_eq!(table.orphan_count(), 0);
    }

    #[test]
    fn native_indices_resolve_both_ways() {
        let ini = IniFile::from_str(
            "[InfantryTypes]\n0=E1\n1=E2\n\n[VehicleTypes]\n0=DEST\n\n\
             [SuperWeaponTypes]\n0=NukeSpecial\n1=IronCurtainSpecial\n\n\
             [E1]\nStrength=100\n[E2]\nStrength=100\n[DEST]\nNaval=yes\n\
             [NukeSpecial]\n[IronCurtainSpecial]\n",
        );
        let rules = RuleSet::from_ini(&ini).expect("ruleset should parse");
        let mut interner = StringInterner::new();
        let e2 = interner.intern("e2");
        let dest = interner.intern("DEST");
        let iron = interner.intern("IronCurtainSpecial");
        let table = TypeHandleTable::build(&rules, &interner);
        assert_eq!(
            table.native_techno(e2),
            Some(NativeTechnoType {
                category: ObjectCategory::Infantry,
                array_index: 1,
                naval: false,
            })
        );
        assert_eq!(
            table.native_techno(dest).map(|native| native.naval),
            Some(true)
        );
        assert_eq!(table.techno_at(ObjectCategory::Infantry, 1), Some(e2));
        assert_eq!(table.techno_at(ObjectCategory::Vehicle, 0), Some(dest));
        // E1 was never interned, so no id names index 0.
        assert_eq!(table.techno_at(ObjectCategory::Infantry, 0), None);
        assert_eq!(table.super_weapon_index(iron), Some(1));
        assert_eq!(table.super_weapon_at(1), Some(iron));
        assert_eq!(table.super_weapon_at(0), None);
        assert_eq!(table.native_techno(iron), None);
    }

    #[test]
    fn unknown_interned_id_is_orphan() {
        let rules = tiny_rules();
        let mut interner = StringInterner::new();
        let bogus = interner.intern("NOSUCHTYPE");
        let table = TypeHandleTable::build(&rules, &interner);
        assert_eq!(table.handle_for(bogus), None);
        assert_eq!(table.orphan_count(), 1);
    }

    #[test]
    fn empty_table_reports_empty() {
        assert!(TypeHandleTable::default().is_empty());
        assert_eq!(
            TypeHandleTable::default().handle_for(InternedId::from_index(0)),
            None
        );
    }
}
