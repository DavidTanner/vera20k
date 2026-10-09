//! Next/Previous536610/536A80 and HealthNav536950 ->733380.
//! Executed N/M controls: tools/input_oracle/selection_navigation.json.

use super::{AppState, SelectionMutation};
use crate::assets::csf_file::{CsfArg, format_csf};
use crate::map::entities::EntityCategory;

#[cfg(test)]
#[path = "selection_navigation_tests.rs"]
mod object_tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ObjectDirection {
    Next,
    Previous,
}

/// Display4AA2B0/4AA380 walk retained Ground/Air/Top vectors, reversing
/// both the layer and member order for Previous. No navigation cache or sort.
fn next_object(
    sim: &crate::sim::world::Simulation,
    rules: Option<&crate::rules::ruleset::RuleSet>,
    anchor: Option<u64>,
    direction: ObjectDirection,
) -> Option<u64> {
    use crate::sim::world::display_layers::DisplayLayer;
    let layers = sim.display_layers();
    let ids = layers
        .members(DisplayLayer::GROUND)
        .iter()
        .chain(layers.members(DisplayLayer::AIR))
        .chain(layers.members(DisplayLayer::TOP))
        .copied();
    let eligible = |id| {
        let Some(entity) = sim.entities().get(id) else {
            return false;
        };
        crate::app::input::entity_pick::is_local_player_selectable_object(sim, rules, entity)
            && crate::app::input::entity_pick::selection_dynamic_prefix(
                entity,
                sim.entities(),
                rules,
                Some(&sim.interner),
            )
            && sim.object_is_selectable(id, rules)
    };
    match direction {
        ObjectDirection::Next => after_anchor(ids, anchor, eligible),
        ObjectDirection::Previous => after_anchor(ids.rev(), anchor, eligible),
    }
}

fn after_anchor(
    ids: impl Iterator<Item = u64>,
    anchor: Option<u64>,
    mut eligible: impl FnMut(u64) -> bool,
) -> Option<u64> {
    let mut first = None;
    let mut passed_anchor = anchor.is_none();
    for id in ids {
        if !eligible(id) {
            continue;
        }
        first.get_or_insert(id);
        if passed_anchor {
            return Some(id);
        }
        passed_anchor = anchor == Some(id);
    }
    // Missing and ineligible anchors wrap just like the final eligible member.
    first
}

pub(super) fn execute_object_navigation(state: &mut AppState, direction: ObjectDirection) {
    let placement = state.armed_building_type().is_some();
    state
        .match_state
        .match_presentation
        .sidebar_gadget_state
        .disable_selection_modes(placement);
    let Some(sim) = state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|rt| &rt.simulation)
    else {
        return;
    };
    let current = super::selected_stable_ids_in_order(
        Some(sim),
        state.rules(),
        &state.match_state.input.selection_order,
        state.match_state.input.selection_order_pending,
    );
    let Some(id) = next_object(sim, state.rules(), current.first().copied(), direction) else {
        return;
    };
    // UnselectAll is unconditional after a found candidate, including a
    // singleton wrap. Select may refuse (e.g. pending building placement).
    super::apply_selection_mutation(
        state,
        SelectionMutation {
            clear: true,
            select: vec![id],
            ..Default::default()
        },
        true,
        super::SelectionVoicePolicy::EveryAdded,
    );
    crate::app::input::camera::center_view_on_selection(state);
    state.platform.window.request_redraw();
    // No selected-action-line timer start, Scenario draw or Detach in this path.
}

#[derive(Debug, Default)]
pub(crate) struct HealthNavigation {
    candidates: Vec<u64>,
    category: u8,
}

impl HealthNavigation {
    fn reset(&mut self, mode: &mut crate::app::types::TypeSelectInputState) {
        *self = Self::default();
        *mode = Default::default();
    }
    pub(crate) fn retain(&mut self, alive: impl FnMut(&u64) -> bool) {
        self.candidates.retain(alive);
    }

    /// Fresh entry snapshots before clearing, and starts at critical even when
    /// Shift is held. Later taps retain the snapshot and advance one category.
    fn prepare(&mut self, continuing: bool, selected: Vec<u64>, visible: Vec<u64>) {
        if continuing {
            self.category = (self.category + 1) % 3;
        } else {
            self.candidates = if selected.is_empty() {
                visible
            } else {
                selected
            };
            self.category = 0;
        }
    }
}

/// World replacement invalidates pointer-based native navigation candidates.
/// Stable IDs can be reused by a new/restored world, so clearing only absent
/// IDs is insufficient. Persistent bindings and CursorCheat are untouched.
pub(crate) fn reset_for_world_replacement(input: &mut crate::app::input::state::MatchInputState) {
    input.health_navigation.reset(&mut input.type_select);
    input.selection_order.clear();
    input.selection_order_pending = false;
}

/// 5F5DD0 uses inclusive red/yellow thresholds and live type Strength.
fn health_category(current: i32, strength: i32, red: f64, yellow: f64) -> u8 {
    use crate::util::native_x87::MaskedX87Ordering::Greater;
    let health = crate::sim::components::Health { current };
    // Object5F5DD0: all three TEST AH,41 comparisons include unordered;
    // only the first red branch also requires signed actual > 0.
    // Original whole-function outputs: health_ratio_predicates.json.
    let above_red = health.compare_ratio(strength, red) == Greater;
    if current > 0 && !above_red {
        0
    } else if above_red && health.compare_ratio(strength, yellow) != Greater {
        1
    } else {
        2
    }
}

fn category_key(category: u8) -> &'static str {
    match category {
        0 => "MSG:Critical",
        1 => "MSG:HeavilyDamaged",
        _ => "MSG:Healthy",
    }
}

pub(super) fn execute_health_navigation(state: &mut AppState) {
    let Some(sim) = state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|rt| &rt.simulation)
    else {
        return;
    };
    let current = super::selected_stable_ids_in_order(
        state
            .match_state
            .sim_runtime
            .as_ref()
            .map(|rt| &rt.simulation),
        state.rules(),
        &state.match_state.input.selection_order,
        state.match_state.input.selection_order_pending,
    );
    let continuing = state
        .match_state
        .input
        .type_select
        .health_navigation_continues();
    let local = super::preferred_local_owner_name(state);
    // 731F70 iterates the tactical draw array. The fallback predicate7335F0
    // admits alive locally controlled mobile technos; current selection is
    // copied without applying that fallback predicate (732050).
    let visible = if !continuing && current.is_empty() {
        crate::app::presentation::instances::tactical_screen_entity_encounter_order(state)
            .into_iter()
            .filter(|id| {
                sim.entities().get(*id).is_some_and(|entity| {
                    entity.lifecycle.object_alive
                        && entity.category != EntityCategory::Structure
                        && local.as_deref().is_some_and(|owner| {
                            sim.interner
                                .resolve(entity.owner())
                                .eq_ignore_ascii_case(owner)
                        })
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    let (red, yellow) = state.rules().map_or((0.25, 0.5), |rules| {
        (rules.general.condition_red, rules.general.condition_yellow)
    });
    state
        .match_state
        .input
        .health_navigation
        .prepare(continuing, current, visible);
    let navigation = &state.match_state.input.health_navigation;
    let category = navigation.category;
    let has_candidates = !navigation.candidates.is_empty();
    let mutation = SelectionMutation {
        clear: !continuing || !state.match_state.input.hotkey_modifiers.shift_key(),
        select: navigation
            .candidates
            .iter()
            .copied()
            .filter(|id| {
                sim.entities().get(*id).is_some_and(|entity| {
                    state
                        .rules()
                        .and_then(|rules| rules.object(sim.interner.resolve(entity.type_ref())))
                        .is_some_and(|obj| {
                            health_category(entity.health.current, obj.strength, red, yellow)
                                == category
                        })
                })
            })
            .collect(),
        ..Default::default()
    };
    super::apply_selection_mutation(
        state,
        mutation,
        false,
        super::SelectionVoicePolicy::EveryAdded,
    );
    state
        .match_state
        .input
        .type_select
        .finish_health_navigation(has_candidates);
    // The native direct Select calls do not start the mouse action-line timer.
    super::apply_selection_action_line_policy(state, super::SelectionActionLinePolicy::Preserve);
    let selected = super::selected_stable_ids_in_order(
        state
            .match_state
            .sim_runtime
            .as_ref()
            .map(|rt| &rt.simulation),
        state.rules(),
        &state.match_state.input.selection_order,
        state.match_state.input.selection_order_pending,
    );
    let sim = &state.match_state.sim_runtime.as_ref().unwrap().simulation;
    let mixed = selected.iter().any(|id| {
        sim.entities().get(*id).is_some_and(|entity| {
            state
                .rules()
                .and_then(|rules| rules.object(sim.interner.resolve(entity.type_ref())))
                .is_some_and(|obj| {
                    health_category(entity.health.current, obj.strength, red, yellow) != category
                })
        })
    });
    let label = localized(
        state,
        if mixed {
            "MSG:Mixed"
        } else {
            category_key(category)
        },
    );
    // Native731E29 sums each object's Cost_Of for its own house
    // (`0x00731E2B..0x00731E42`).
    let worth = selected
        .iter()
        .filter_map(|id| sim.entities().get(*id))
        .filter_map(|entity| {
            let rules = state.rules()?;
            let object = rules.object(sim.interner.resolve(entity.type_ref()))?;
            Some(sim.cost_of(entity.owner(), object, rules))
        })
        .fold(0_i32, |total, cost| total.wrapping_add(cost));
    let text = navigation_feedback(
        has_candidates,
        selected.len(),
        worth,
        &label,
        &localized(state, "MSG:NavEmpty"),
        &localized(state, "MSG:NoUnitsSel"),
        &localized(state, "MSG:UnitsWorth"),
    );
    crate::app::input::messages::post_selection_navigation_text(state, &text);
}

fn localized(state: &AppState, key: &str) -> String {
    state
        .process_assets
        .csf
        .as_ref()
        .map_or_else(|| key.to_owned(), |csf| csf.text(key).into_owned())
}

fn navigation_feedback(
    has_candidates: bool,
    count: usize,
    worth: i32,
    category: &str,
    empty: &str,
    none: &str,
    units: &str,
) -> String {
    // 731DC6 -> 7DDCD6 uppercases the category before inserting it. The
    // original C-locale branch changes only ASCII a..z.
    let category = category.to_ascii_uppercase();
    if !has_candidates {
        empty.to_owned()
    } else if count == 0 {
        format_csf(none, &[CsfArg::Str(&category)])
    } else {
        format_csf(
            units,
            &[
                CsfArg::Int(count as i64),
                CsfArg::Str(&category),
                CsfArg::Int(i64::from(worth)),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_health_ratio_corpus_matches_navigation_category() {
        for row in crate::sim::health_ratio_fixture::rows() {
            assert_eq!(
                health_category(
                    row.input.current,
                    row.input.strength,
                    row.input.red(),
                    row.input.yellow()
                ),
                row.output.navigation_category,
                "{row:?}"
            );
        }
    }

    #[test]
    fn cycles_retained_selection_and_restarts_after_other_selection() {
        let mut nav = HealthNavigation::default();
        nav.prepare(false, vec![8, 3, 7], vec![99]);
        assert_eq!((&nav.candidates, nav.category), (&vec![8, 3, 7], 0));
        nav.prepare(true, vec![8], vec![99]);
        assert_eq!((&nav.candidates, nav.category), (&vec![8, 3, 7], 1));
        nav.retain(|id| *id != 3);
        nav.prepare(true, vec![], vec![]);
        assert_eq!((&nav.candidates, nav.category), (&vec![8, 7], 2));
        nav.prepare(true, vec![], vec![]);
        assert_eq!(nav.category, 0);
        nav.prepare(false, vec![], vec![21, 20]);
        assert_eq!(nav.candidates, vec![21, 20]);
    }

    #[test]
    fn health_bands_include_thresholds_and_recheck_current_damage() {
        let bands: Vec<_> = [0, 1, 25, 26, 50, 51, 100]
            .into_iter()
            .map(|hp| health_category(hp, 100, 0.25, 0.5))
            .collect();
        assert_eq!(bands, [2, 0, 0, 1, 1, 2, 2]);
    }

    #[test]
    fn health_bands_keep_signed_actual_and_live_strength_width() {
        assert_eq!(health_category(70_000, 100_000, 0.25, 0.5), 2);
        assert_eq!(health_category(70_000, 200_000, 0.25, 0.5), 1);
        assert_eq!(health_category(70_000, 400_000, 0.25, 0.5), 0);
        assert_eq!(health_category(-1, 100_000, 0.25, 0.5), 2);
        assert_eq!(health_category(i32::MAX, i32::MAX, 0.25, 0.5), 2);
    }

    #[test]
    fn world_replacement_drops_candidates_even_when_ids_are_reused() {
        let mut nav = HealthNavigation::default();
        let mut mode = crate::app::types::TypeSelectInputState::default();
        nav.prepare(false, vec![8, 3], vec![]);
        mode.finish_health_navigation(true);
        nav.reset(&mut mode);
        assert!(nav.candidates.is_empty());
        assert!(!mode.health_navigation_continues());
        nav.prepare(mode.health_navigation_continues(), vec![8], vec![]);
        assert_eq!(nav.candidates, vec![8]);
        assert_eq!(nav.category, 0);
    }

    #[test]
    fn feedback_distinguishes_empty_snapshot_empty_band_and_selection() {
        let render = |has, count| {
            navigation_feedback(
                has,
                count,
                1500,
                "Critical",
                "Empty",
                "No %s",
                "%d %s worth %d",
            )
        };
        assert_eq!(render(false, 0), "Empty");
        assert_eq!(render(true, 0), "No CRITICAL");
        assert_eq!(render(true, 2), "2 CRITICAL worth 1500");
    }
}
