# Build cache retention validation

The shared `cargo_run` owner holds `owned-builds/cargo.lock` across retention,
Cargo, label publication and final retention. `_cargo_cache` is its private
implementation, not a separate cleanup command or lock owner. The immutable
manifest validator is shared with labelled executable resolution; protecting a
library-test label does not make it eligible for host executable resolution.

The policy tests in [test_cargo_cache.py](tests/test_cargo_cache.py) exercise
dry-run nonmutation, budgets, real-free-space fallback, protected dependencies,
all-or-nothing preflight, stale mutable latest records, links, whole sessions,
publication/input races and partial unlink failure. Default repository Python
tests require no compiler, retail data or debugging utilities:

```sh
python -m tools.run_tests
```

The full final Python suite passed369 tests (3 optional tests skipped).
The focused final suite passed62 tests:40 retention tests and22 existing runner
tests. The opt-in native test passed separately. Saved log identities are in the
receipt. The single fresh critic found discovery-window, nested-layout and
free-space-error defects. Five focused tests first reproduced those failures;
the corrected owner passes them, and two further tests bound cross-target profile
admission. Directory identities are captured before enumeration, including
intermediate directories. Nested research/build-script outputs never qualify as
compiler cache profiles. Unavailable final free-space samples remain explicit in
the blocked/partial receipt.

[The opt-in compiled test](tests/test_cargo_cache_native.py) creates a temporary
Git repository and owned cache, compiles a real debug object and binary, preserves
a schema-1 library-test label, and puts an orphan object beside the required one.
It acquires the actual project's build lock before invoking the compiler. Run:

```sh
VERA20K_CACHE_NATIVE_TEST=1 python -m unittest tools.tests.test_cargo_cache_native
```

On macOS arm64 / Apple clang17, the native test passed: dry-run left all files
unchanged, apply removed the orphan, source/binary/required-object hashes matched,
the preserved binary printed42, and its debugging dependencies were unchanged.
After trimming, `dsymutil` built a dSYM containing `required_answer` and `fixture.c`.
[The saved receipt](cargo_cache_validation.json) retains compiler identity,
hashes, selected/removed bytes and observed volume changes. Temporary fixture
paths identify that run and are deleted by the test; the harness reproduces it.
This validates the host example, not gamemd or whole-game debugging equivalence.

The real project dry-run returned `blocked` and removed **zero** files. An older
preserved bridge label references already missing objects in another worktree.
The inspector exits0 but emits warnings and omits those missing inputs from its
YAML. The receipt retains that failure. No label, source, retail asset or native
evidence was removed, and no production apply was attempted. Repair or explicitly
retire those older validation dependencies before expecting this store to trim.

Mach-O parsing follows LLVM's
[debug-map schema](https://github.com/llvm/llvm-project/blob/release/17.x/llvm/tools/dsymutil/DebugMap.cpp),
[missing-object behavior](https://github.com/llvm/llvm-project/blob/release/17.x/llvm/tools/dsymutil/MachODebugMapParser.cpp)
and [archive loading](https://github.com/llvm/llvm-project/blob/release/17.x/llvm/tools/dsymutil/BinaryHolder.cpp).
Output is streamed; any warning, failure or malformed/truncated document blocks
deletion. Dependencies must exist as regular files. Thin archive external members
are unsupported. Empty maps from ordinary stripped release binaries are valid.

ELF scanning uses [GNU readelf](https://sourceware.org/binutils/docs/binutils/readelf.html)
to inspect sections and all compilation units without following debug links or
contacting debuginfod. Embedded DWARF may permit trimming; split/external debug
information blocks it. [Separate debug files](https://www.sourceware.org/gdb/current/onlinedocs/gdb.html/Separate-Debug-Files.html)
need identity/CRC and external closure support that is not implemented here.
PE/PDB closure also blocks trimming: [MSVC FASTLINK](https://learn.microsoft.com/en-us/cpp/build/reference/debug-generate-debug-info?view=msvc-170)
may depend on original objects/libraries beyond the PDB. Those formats retain
normal Cargo behavior and make no unsupported deletion claim.

Allocated bytes removed and observed free-space delta are distinct. The native
receipt's small deletion did not imply a positive volume delta: receipt writes,
APFS allocation and concurrent unrelated activity also affect free space.
Dry-run projections are estimates; apply checks actual free space as it proceeds,
then records remaining protected-budget and free-space shortfalls.
