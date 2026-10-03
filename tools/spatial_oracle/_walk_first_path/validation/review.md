# Single fresh read-only GI review

Reviewed HEADff7d8f154b8585ba5f86b8e72251559875092f2e by independent agent
`gi_move_owner_687_critic` after actual validation and before PR publication.
No build, replay, edits or publication were performed by the critic. This is the
single pass; it is not a certificate for the broader movement/height audit.

## Critic report

No confirmed implementation defect in the scoped GI change. One pre-existing
adjacent defect belongs in the remaining audit:

**[P2] Cancelled boarding retains a competing destination.** EnterTransport stores
`PassengerRole::Boarding.target_transport_id`; subsequent Move clears attack/order
intent and replaces NavCom, but retains that role. `passenger.rs:503` continues
dispatching boarding from it, and `process_boarding_passenger` checks the retained
transport and proximity without checking mission or NavCom. A GI given Move after
Enter can consequently board the old transport when it later approaches it.
Original active-retail InfantryPerCell `51A21F..51A252` requires Enter7 and the live
NavCom/TarCom/object relationship before entering its boarding branches. This
predates the diff; it is a follow-up for the documented separate PerCell boarding
migration, rather than a new destination-dispatch regression. No runtime
reproduction was run during this read-only pass.

The critic inspected all production Rust changes, their tests, the four app Cell
producers, shared retasking, FindPath redirects, Capture/boarding coordinates, and
internal ejection. Independent original-byte reads support the distinct House
predicates, Event Archive ordering, non-Event ejection queues, and virtual FindPath
destination calls. The change reuses existing class and Foot/Walk owners and
preserves the paid head separately.

All referenced native and validation archive entries passed independent size/SHA/
gzip checks. The21 archived Rust leaves match source. Actual logs contain9662/0
retail lib,63/0/no skips, successful Clippy/release completion, and582 Python tests/
5optional skips.

Coverage is bounded: paid AI state is imported into Rust; runtime uses decoded
commands; full physical input, whole native pathfinding/AI, all boarding arrival
branches, and remaining Unit/Jumpjet/Fly migrations are not certified. The packet
states these limits accurately.

## Owner disposition

No implementation defect required a fix. The owner confirmed the live retained-role
reader and ordinary Move writer, then independently read original519630 through
51A258. The3112-byte packet decodes completely with pinned native identity. This
establishes the mission/identity gates; it is not a boarding execution comparison.

The retained passenger target is tracked in [#1045](https://github.com/YuriPlanet/vera20k/issues/1045)
after searching open boarding/PerCell/PassengerRole/passenger issues. Its complete
native PerCell arrival/retask/lifecycle mechanism requires its own original caller
controls and belongs to the continuing goal. This PR changes the transport reference
passed into the class setter; it does not migrate that larger boarding operation.
Trigger: EnterTransport→Move→approach old transport. Effect/risk: cancelled boarding
can consume the GI and alter cargo/gunner state. Frequency: this cancellation route.
The broader audit remains open. No repeat critic was requested.
