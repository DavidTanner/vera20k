"""Machine-derived RNG vectors: seeded state, raw and mixed ranged histories.

Run with ``python -m tools.rmg_oracle.gen_rng_vectors`` (read-only check by default).
Runs the real gamemd.exe routines under Unicorn (see tools.native_oracle):
  Random__Seed 0x0065C6D0  __thiscall(this=ECX, seed=stack) -> fills this+0xC..
  Random__Next 0x0065C780  __thiscall(this=ECX) -> EAX, mutates state in place
  Random__RandomRanged 0x0065C7E0  __thiscall(this=ECX, low, high) -> signed EAX

The original seed/raw cases retain their schema. Added ranged cases execute
mixed calls with full state retained; observe raw words at the actual native
stores (65C79D/65C84B), including RandomRanged's inlined draw and rejection.
No expected-value reduction or returned-function substitution is used.
"""

import struct
from pathlib import Path

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import (
    UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESI, UC_X86_REG_ESP,
)

from tools.native_oracle import (
    RET_MAGIC, SCRATCH, SCRATCH_SIZE, STACK_BASE, STACK_SIZE,
    call, finish_vectors, load_image, provenance, run_checked,
)

SEED_FN = 0x0065C6D0
NEXT_FN = 0x0065C780
RANGED_FN = 0x0065C7E0
STRUCT = SCRATCH
STRUCT_LEN = 0xC + 250 * 4  # locked/idx_a/idx_b + 250 state dwords = 0x3F4

SEEDS = (0, 1, 1234, 0x7FFF, 0xFFFF)
DRAWS_PER_SEED = 16


def seeded_struct(seed: int) -> bytes:
    """Run Random__Seed and return the full 0x3F4-byte generator struct."""
    result = call(
        SEED_FN,
        ecx=STRUCT,
        stack_args=[seed],
        dumps={"s": (STRUCT, STRUCT_LEN)},
    )
    return bytes.fromhex(result["dumps"]["s"])


def draws(state: bytes, count: int) -> tuple[list[int], bytes]:
    """Chain `count` calls to Random__Next, carrying the struct forward."""
    current, values = state, []
    for _ in range(count):
        result = call(
            NEXT_FN,
            ecx=STRUCT,
            writes={STRUCT: current},
            dumps={"s": (STRUCT, STRUCT_LEN)},
        )
        values.append(result["eax"])
        current = bytes.fromhex(result["dumps"]["s"])
    return values, current


def ranged_history(seed: int, advance_raw: int = 0) -> dict:
    """Execute unchanged original routines, observing results and draw stores.

    The fixture supplies the RNG object address and native-produced seed bytes.
    A raw prefix tests either cursor wrapping. Prefix calls execute but are not
    repeated as step receipts; initial_state_hex is their actual continuation.
    """
    uc = Uc(UC_ARCH_X86, UC_MODE_32)
    load_image(uc)
    uc.mem_map(STACK_BASE, STACK_SIZE)
    uc.mem_map(SCRATCH, SCRATCH_SIZE)
    uc.mem_map(RET_MAGIC, 0x1000)
    uc.mem_write(STRUCT, seeded_struct(seed))
    original_code = bytes(uc.mem_read(NEXT_FN, 0x65C88D - NEXT_FN))
    raw_words = []

    def observe(machine, address, _size, _user):
        # Both routines store the unmasked XOR result from ESI. Count this
        # instruction rather than assuming one draw or deriving it from EAX.
        if address in (0x65C79D, 0x65C84B):
            raw_words.append(machine.reg_read(UC_X86_REG_ESI) & 0xFFFFFFFF)

    uc.hook_add(UC_HOOK_CODE, observe)
    sp = STACK_BASE + STACK_SIZE - 0x1000

    def request(bounds):
        raw_words.clear()
        before = bytes(uc.mem_read(STRUCT, STRUCT_LEN))
        function = NEXT_FN if bounds is None else RANGED_FN
        stack = [RET_MAGIC] + ([] if bounds is None else list(bounds))
        uc.mem_write(sp, struct.pack("<" + "I" * len(stack),
                                     *(value & 0xFFFFFFFF for value in stack)))
        uc.reg_write(UC_X86_REG_ESP, sp)
        uc.reg_write(UC_X86_REG_ECX, STRUCT)
        required = [function, 0x65C7D0 if bounds is None else 0x65C88A]
        if bounds is not None and bounds[0] != bounds[1]:
            required.append(0x65C84B)
        run_checked(uc, function, RET_MAGIC, count=10000,
                    required_addresses=required,
                    context=dict(seed=seed, advance_raw=advance_raw, bounds=bounds))
        assert uc.reg_read(UC_X86_REG_ESP) == sp + len(stack) * 4
        result = uc.reg_read(UC_X86_REG_EAX) & 0xFFFFFFFF
        row = dict(kind="raw" if bounds is None else "ranged",
                   result=result if bounds is None else struct.unpack("<i", struct.pack("<I", result))[0],
                   raw_draw_count=len(raw_words), raw_draws=raw_words.copy(),
                   before_state_hex=before.hex(),
                   after_state_hex=bytes(uc.mem_read(STRUCT, STRUCT_LEN)).hex())
        if bounds is not None:
            row.update(low=bounds[0], high=bounds[1])
        assert bytes(uc.mem_read(NEXT_FN, len(original_code))) == original_code
        return row

    for _ in range(advance_raw):
        request(None)
    # Seed1's first [0,4] request requires three draws: a useful distinction
    # from modulo reduction. Equal and reversed bounds share the live cursor.
    requests = ((0, 4), None, (0, 99), (-5, 5), None, (5, -5), (7, 7),
                None, (0, 99), (99, 0), (-5, -5), (0, 1), (0, 2), None)
    initial = bytes(uc.mem_read(STRUCT, STRUCT_LEN)).hex()
    return dict(id=f"seed_{seed}_after_{advance_raw}_raw", seed=seed,
                advance_raw=advance_raw, initial_state_hex=initial,
                steps=[request(bounds) for bounds in requests])


def generate() -> dict:
    """Preserve legacy seed/raw cases and add original-executed mixed calls."""
    vectors = {
        "source": "unicorn/gamemd.exe",
        "seed_fn": hex(SEED_FN),
        "next_fn": hex(NEXT_FN),
        "ranged_fn": hex(RANGED_FN),
        "struct_len": STRUCT_LEN,
        "cases": [],
    }
    for seed in SEEDS:
        blob = seeded_struct(seed)
        locked = blob[0]
        idx_a, idx_b = struct.unpack_from("<II", blob, 4)
        values, _ = draws(blob, DRAWS_PER_SEED)
        vectors["cases"].append(
            {
                "seed": seed,
                "locked": locked,
                "idx_a": idx_a,
                "idx_b": idx_b,
                "state_hex": blob[0xC:].hex(),
                "draws": [f"{v:08x}" for v in values],
            }
        )
    vectors["ranged_cases"] = [ranged_history(seed) for seed in SEEDS]
    vectors["ranged_cases"].extend(ranged_history(1, prefix) for prefix in (146, 249))
    return vectors


def main() -> None:
    finish_vectors(
        generate,
        Path(__file__).parent / "vectors" / "rng.json",
        provenance=lambda: dict(provenance(
            scope=("Random seed and first 16 draws for five selected seeds, plus seven "
                   "mixed raw/ranged histories (98 calls) at ordinary audio bounds. "
                   "Includes equal/reversed bounds, rejection and cursor-wrap continuation; "
                   "not exhaustive signed-range or full generator parity."),
            assumptions=[
                "A zero-initialized 0x3f4-byte generator is supplied to Random__Seed.",
                "Legacy raw calls use fresh emulators and carry only generator bytes forward.",
                "Each mixed history retains one emulator and native-produced RNG state; optional 146/249 raw prefixes execute before the recorded initial state.",
                "Source RNG initialization and supplied bounds are fixture inputs, not a whole application audio/theme scheduling claim.",
                "Raw draw counts and values are observed at native word stores65C79D/65C84B; RandomRanged inlines the draw instead of calling65C780.",
                "Disabled RNG objects and signed spans at or above 0x7fffffff are outside these controls.",
            ],
            substitutions=[],
            entry_points={"Random__Seed": SEED_FN, "Random__Next": NEXT_FN,
                          "Random__RandomRanged": RANGED_FN},
        ), command="python -m tools.rmg_oracle.gen_rng_vectors --check"),
        source_paths={"generator": Path(__file__),
                      "checked_runner": Path(__file__).parents[1] / "native_oracle.py"},
    )


if __name__ == "__main__":
    main()
