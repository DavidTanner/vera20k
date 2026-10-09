//! The production save envelope and candidate transaction retain strip history.
//! Native lifecycle: Mouse Save5BE6D0 / Load5BDF70 / no-init5BE9B0.

use super::*;
use crate::app::sidebar_projection::SidebarProjectionState;
use crate::sim::intern::InternedId;
use crate::sim::snapshot::{SavedCameoId, SavedSidebarOrder};
use crate::ui::sidebar::SidebarTab;
use crate::ui::sidebar::cameo_order::{Cameo, CameoKey};

fn fixture() -> (Simulation, RuleSet, InternedId, [InternedId; 2]) {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[Countries]\n0=Americans\n[Americans]\nSide=GDI\n\
         [BuildingTypes]\n0=FIRST\n1=LATER\n\
         [FIRST]\nCost=400\nStrength=1000\nFoundation=1x1\nTechLevel=1\nOwner=Americans\n\
         [LATER]\nCost=200\nStrength=1000\nFoundation=1x1\nTechLevel=1\nOwner=Americans\n",
    ))
    .unwrap();
    let mut sim = load_fixture_simulation(true);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    let owner = sim.interner.intern("Americans");
    sim.houses.insert(
        owner,
        crate::sim::house_state::HouseState::new(owner, 0, None, true, 50_000, 10),
    );
    sim.session.house_order.push(owner);
    sim.session.current_house = Some(owner);
    sim.scenario_rng = crate::sim::rng::SimRng::new(0);
    let types = [
        sim.interner.get("FIRST").unwrap(),
        sim.interner.get("LATER").unwrap(),
    ];
    (sim, rules, owner, types)
}

fn candidates(types: [InternedId; 2], first_cost: i32) -> Vec<Cameo> {
    types
        .into_iter()
        .zip([first_cost, 200])
        .map(|(id, cost)| Cameo {
            id: SavedCameoId::Object(id),
            tab: SidebarTab::Building,
            available: true,
            key: CameoKey::Techno {
                own_side: true,
                considered_aircraft: false,
                naval: false,
                tech_level: 1,
                cost,
                name: String::new(),
            },
        })
        .collect()
}

fn retained_history(owner: InternedId, types: [InternedId; 2]) -> SidebarProjectionState {
    let mut projection = SidebarProjectionState::default();
    assert!(!projection.reconcile_cameos(owner, &candidates(types, 100)));
    // The current keys would sort LATER first, but native Recalculate retains
    // survivors. This records history through the actual shared strip owner.
    assert!(!projection.reconcile_cameos(owner, &candidates(types, 400)));
    assert_eq!(
        projection.saved_order().unwrap().strips()[0],
        types.map(SavedCameoId::Object)
    );
    projection
}

fn save(sim: &Simulation, rules: &RuleSet, order: Option<&SavedSidebarOrder>) -> Vec<u8> {
    GameSnapshot::save_validated_with_presentation(
        sim,
        LOAD_FIXTURE_MAP_HASH,
        rules.simulation_config_hash(),
        "retained sidebar",
        1,
        &[(7, -12)],
        order,
    )
}

fn prepare(
    bytes: &[u8],
    sim: &Simulation,
    rules: &RuleSet,
) -> Result<PreparedLoad, PreparedLoadError> {
    PreparedLoad::prepare_candidate(
        bytes,
        Some(sim),
        Some(LOAD_FIXTURE_MAP_HASH),
        Some(rules),
        Some(&load_fixture_terrain()),
        Some(&OverlayTypeRegistry::empty()),
    )
}

#[test]
fn sidebar_order_envelope_roundtrips_without_changing_simulation_hash() {
    let (sim, rules, owner, types) = fixture();
    let saved = retained_history(owner, types).saved_order().unwrap();
    let hash = sim.state_hash();
    let bytes = save(&sim, &rules, Some(&saved));
    let loaded = GameSnapshot::load_validated(
        &bytes,
        LOAD_FIXTURE_MAP_HASH,
        rules.simulation_config_hash(),
        LOAD_FIXTURE_MAP_NAME,
    )
    .unwrap();
    assert_eq!(loaded.sidebar_order(), Some(&saved));
    assert_eq!(loaded.sidebar_order().unwrap().owner(), owner);
    assert_eq!(loaded.sinking_waterlines, [(7, -12)]);
    assert_eq!(loaded.sim.state_hash(), hash);
    let bytes = GameSnapshot::save_validated(
        &sim,
        LOAD_FIXTURE_MAP_HASH,
        rules.simulation_config_hash(),
        "headless",
        1,
    );
    let loaded = GameSnapshot::load_validated(
        &bytes,
        LOAD_FIXTURE_MAP_HASH,
        rules.simulation_config_hash(),
        LOAD_FIXTURE_MAP_NAME,
    )
    .unwrap();
    assert!(loaded.sidebar_order().is_none());
    assert!(loaded.sinking_waterlines.is_empty());
    assert_eq!(loaded.sim.state_hash(), hash);
}

#[test]
fn sidebar_order_prepares_and_commits_unsorted_history_without_reinserting_survivors() {
    let (sim, rules, owner, types) = fixture();
    let saved = retained_history(owner, types).saved_order().unwrap();
    let bytes = save(&sim, &rules, Some(&saved));
    let mut outgoing = SidebarProjectionState::default();
    assert!(!outgoing.reconcile_cameos(owner, &candidates(types, 400)));
    let before = outgoing.saved_order().unwrap();
    assert_ne!(before, saved);
    let prepared = prepare(&bytes, &sim, &rules).unwrap();
    assert_eq!(outgoing.saved_order(), Some(before));
    let entries = prepared
        .sidebar_order
        .as_ref()
        .unwrap()
        .1
        .items(SidebarTab::Building);
    assert_eq!(
        entries.iter().map(|item| item.id).collect::<Vec<_>>(),
        saved.strips()[0]
    );
    assert!(matches!(entries[0].key, CameoKey::Techno { cost: 400, .. }));
    assert!(matches!(entries[1].key, CameoKey::Techno { cost: 200, .. }));
    let mut runtime = crate::sim::runtime::SimRuntime::from_simulation(sim);
    let committed = prepared.commit_into(&mut runtime);
    outgoing.restore_order(committed.sidebar_order);
    assert_eq!(outgoing.saved_order(), Some(saved.clone()));
    assert!(!outgoing.reconcile_cameos(owner, &candidates(types, 400)));
    assert_eq!(outgoing.saved_order(), Some(saved));

    // A headless save has no retained UI history: replacing with it resets
    // the outgoing order, and the next normal seed remains silent.
    let headless = save(&runtime.simulation, &rules, None);
    let prepared = prepare(&headless, &runtime.simulation, &rules).unwrap();
    let committed = prepared.commit_into(&mut runtime);
    outgoing.restore_order(committed.sidebar_order);
    assert!(outgoing.saved_order().is_none());
    assert!(!outgoing.reconcile_cameos(owner, &candidates(types, 400)));
}

#[test]
fn invalid_sidebar_payload_preserves_outgoing_projection_and_running_world() {
    let (sim, rules, owner, types) = fixture();
    let outgoing = retained_history(owner, types);
    let before = outgoing.saved_order();
    let hash = sim.state_hash();
    let rng = sim.rng_state();
    let valid = before.clone().unwrap();
    let mut duplicate = valid.strips().clone();
    let first = duplicate[0][0];
    duplicate[0].push(first);
    let mut wrong_strip = std::array::from_fn(|_| Vec::new());
    wrong_strip[3].push(SavedCameoId::Object(types[0]));
    let mut unknown = valid.strips().clone();
    unknown[0][0] = SavedCameoId::Object(InternedId::from_index(u32::MAX));
    let mut wrong_kind = valid.strips().clone();
    wrong_kind[0][0] = SavedCameoId::SuperWeapon(types[0]);
    let mut over_capacity = std::array::from_fn(|_| Vec::new());
    over_capacity[0] = vec![SavedCameoId::Object(types[0]); 77];
    let invalid = [
        SavedSidebarOrder::new(InternedId::from_index(u32::MAX), valid.strips().clone()),
        SavedSidebarOrder::new(owner, duplicate),
        SavedSidebarOrder::new(owner, wrong_strip),
        SavedSidebarOrder::new(owner, unknown),
        SavedSidebarOrder::new(owner, wrong_kind),
        SavedSidebarOrder::new(owner, over_capacity),
    ];
    for order in invalid {
        assert!(matches!(
            prepare(&save(&sim, &rules, Some(&order)), &sim, &rules),
            Err(PreparedLoadError::SidebarOrder(_))
        ));
        assert_eq!(outgoing.saved_order(), before);
        assert_eq!(sim.state_hash(), hash);
        assert_eq!(sim.rng_state(), rng);
    }
}
