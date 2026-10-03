# CLEG destination critic — accepted finding

Condensed record of the ONE fresh read-only pass by /root/cleg_destination_critic on candidate9303cd6b4c0191026f67c15c701ab258c856ef60, before publication. No second review is requested after repairs.

One confirmed blocking P1: caller overlay registry is lost before Teleport Infantry admission. In ground_move.rs156–160, ordinary Move receives context at world_commands.rs572 but calls set_infantry_destination with None. teleport_movement.rs425 calls Infantry admission after raw release and resolved-coordinate publication. object_entry.rs1472–1476 errors on an overlay without a registered type, leaving a partial reservation transaction and skipping physical restore/recursiveNULL/Foot timer tail.

The same missing prerequisite affects restoration callbacks: mission/authority.rs739/760 construct providers with None, infantry_scatter.rs531–538 accepts nonmoving Teleport on terrain presence alone, concrete_effects.rs320–326 then expects setter success. track_path.rs812/825 has the same checked-inputs guarantee. Repair the caller graph, existing dependency availability owner, and post-release error cleanup. Add focused regression before repair where practical.

Existing source/payload bindings, native22cases/46boundaries, shared481180 consolidation, Teleport instance ownership, snapshot/hash migration and observer filtering produced no other confirmed blocking defect. Existing controls are overlay-free; their original validity does not establish overlay admission. New native overlay controls must be independently executed or covered by existing executed Infantry evidence.

Status: P1 accepted; Root and scope_audit are repairing, null_native is extending the existing original controls with2 overlay cases. Earlier passing validation/runtime belong to the pre-repair candidate until revalidated.
