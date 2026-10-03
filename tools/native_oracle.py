"""Shared, bounded execution and reference-file workflow for retail YR oracles.

This is a CPU fixture runner, not a Windows loader or a game environment.
See tools/native_oracle.md for supported workflows and evidence limits.
Unicorn 2.1.4 API: https://github.com/unicorn-engine/unicorn/blob/2.1.4/include/unicorn/unicorn.h
"""

from __future__ import annotations

import argparse
from collections import deque
from functools import lru_cache
import hashlib
import json
import os
from pathlib import Path
import struct
import uuid

import unicorn
from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_QUERY_TIMEOUT
from unicorn.x86_const import (
    UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX,
    UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_ESP, UC_X86_REG_EBP,
    UC_X86_REG_EIP, UC_X86_REG_EFLAGS, UC_X86_REG_FPCW,
)

NATIVE_SHA256 = "1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c"
IMAGE_BASE = 0x00400000
# Preserve the original fixture mapping, including runtime globals in BSS.
IMAGE_SIZE = 0x00A00000
STACK_BASE, STACK_SIZE = 0x10000000, 0x00100000
SCRATCH, SCRATCH_SIZE = 0x20000000, 0x00010000
RET_MAGIC = 0x30000000
# Legacy RMG fixture contract: 53-bit precision, truncate. A caller must establish
# the appropriate ambient state for its own native entry; this is not universal.
NATIVE_FPCW = 0x0E7F


class OracleError(RuntimeError):
    """No trustworthy result was obtained; do not publish reference outputs."""


class NativeExecutionError(OracleError):
    """An unsuccessful run with captured evidence, never a native result."""

    def __init__(self, message: str, diagnostics: dict):
        self.diagnostics = diagnostics
        self.report_path = None
        directory = os.environ.get("VERA20K_NATIVE_FAILURE_DIR")
        if directory:
            try:
                parent = Path(directory).expanduser().resolve()
                parent.mkdir(parents=True, exist_ok=True)
                path = parent / f"native-failure-{uuid.uuid4().hex}.json"
                # Never replace a golden or an earlier failure, including when
                # several processes use the same diagnostic directory.
                with path.open("x", encoding="utf-8") as output:
                    output.write(json.dumps(diagnostics, indent=2, allow_nan=False) + "\n")
                self.report_path = path
                message += f"; failure report: {path}"
            except (OSError, ValueError) as error:
                message += f"; Could not save failure report: {error}"
        super().__init__(message)


def _machine_state(uc: Uc) -> dict:
    """Read a bounded diagnostic snapshot without mapping or changing memory."""
    registers = {}
    unavailable = {}
    for name, register in (
        ("eax", UC_X86_REG_EAX), ("ebx", UC_X86_REG_EBX),
        ("ecx", UC_X86_REG_ECX), ("edx", UC_X86_REG_EDX),
        ("esi", UC_X86_REG_ESI), ("edi", UC_X86_REG_EDI),
        ("esp", UC_X86_REG_ESP), ("ebp", UC_X86_REG_EBP),
        ("eip", UC_X86_REG_EIP), ("eflags", UC_X86_REG_EFLAGS),
        ("fpcw", UC_X86_REG_FPCW),
    ):
        try:
            registers[name] = uc.reg_read(register)
        except UcError as error:
            unavailable[name] = str(error)
    stack = {"address": registers.get("esp"), "words": [], "requested_words": 16}
    if stack["address"] is not None:
        for index in range(stack["requested_words"]):
            address = stack["address"] + 4 * index
            if address + 4 > 0x100000000:
                stack["unavailable"] = "Stack sample reached the end of the x86 address space"
                break
            try:
                stack["words"].append(struct.unpack("<I", uc.mem_read(address, 4))[0])
            except UcError as error:
                stack["unavailable"] = str(error)
                break
    else:
        stack["unavailable"] = "ESP unavailable"
    return {"registers": registers, "unavailable_registers": unavailable, "stack": stack}


def configured_gamemd() -> Path:
    explicit = os.environ.get("VERA20K_GAMEMD_EXE")
    retail_dir = os.environ.get("RA2_DIR")
    if not explicit and not retail_dir:
        raise OracleError("Set VERA20K_GAMEMD_EXE or RA2_DIR to the original retail gamemd.exe")
    path = Path(explicit) if explicit else Path(retail_dir) / "gamemd.exe"
    path = path.expanduser().resolve()
    if not path.is_file():
        raise OracleError(f"Missing original executable: {path}")
    return path


@lru_cache(maxsize=2)
def _verified_image(path: Path) -> bytes:
    # Hash the same immutable bytes subsequently mapped, not a separate read.
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest != NATIVE_SHA256:
        raise OracleError(f"Unsupported gamemd.exe SHA-256 {digest}; expected {NATIVE_SHA256}")
    return data


def image_bytes() -> bytes:
    return _verified_image(configured_gamemd())


def _sections(data: bytes):
    if len(data) < 0x40:
        raise OracleError("Truncated PE DOS header")
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if pe < 0x40 or pe + 24 > len(data):
        raise OracleError("PE header offset is outside the file")
    machine, count = struct.unpack_from("<HH", data, pe + 4)
    optional_size = struct.unpack_from("<H", data, pe + 20)[0]
    optional = pe + 24
    if optional_size < 32 or optional + optional_size > len(data):
        raise OracleError("Truncated PE32 optional header")
    if optional + optional_size + count * 40 > len(data):
        raise OracleError("Truncated PE section table")
    if (data[:2] != b"MZ" or data[pe:pe + 4] != b"PE\0\0" or machine != 0x14C
            or struct.unpack_from("<H", data, optional)[0] != 0x10B
            or struct.unpack_from("<I", data, optional + 28)[0] != IMAGE_BASE):
        raise OracleError("Expected the pinned PE32 x86 image at 0x00400000")
    for index in range(count):
        offset = optional + optional_size + index * 40
        virtual_size, rva, raw_size, raw_ptr = struct.unpack_from("<IIII", data, offset + 8)
        flags = struct.unpack_from("<I", data, offset + 36)[0]
        if rva + max(virtual_size, raw_size) > IMAGE_SIZE or raw_ptr + raw_size > len(data):
            raise OracleError("PE section exceeds the verified fixture mapping")
        yield rva, raw_ptr, raw_size, virtual_size, flags


def file_span(data: bytes, va: int, size: int) -> tuple[int, bytes]:
    """Resolve one original file-backed section span, never synthesized memory.

    The input bytes must be identity-checked by the caller for native claims.
    Unlike load_image's zero-filled mapping, headers, gaps, BSS and requests
    crossing a section boundary have no supported file span. Section raw padding
    remains readable exactly as load_image maps it, even beyond VirtualSize.
    """
    if size <= 0:
        raise OracleError("PE file span size must be positive")
    start = va - IMAGE_BASE
    end = start + size
    if start < 0 or end > IMAGE_SIZE:
        raise OracleError("PE file span is outside the fixture image")
    # Validate every section before returning data, including malformed sections
    # after the requested one. _sections remains the single PE parsing owner.
    sections = list(_sections(data))
    intersecting = [section for section in sections
                    if start < section[0] + max(section[2], section[3])
                    and end > section[0]]
    if len(intersecting) == 1:
        rva, raw_ptr, raw_size, _, _ = intersecting[0]
        if rva <= start and end <= rva + raw_size:
            offset = raw_ptr + start - rva
            return offset, bytes(data[offset:offset + size])
    raise OracleError(
        f"PE file span 0x{va:08X}+{size} must lie within one file-backed section "
        "(not headers, gaps, BSS or a section crossing)")


def load_image(uc: Uc) -> None:
    """Map verified original PE sections, zero-fill gaps/BSS; no OS initialization.

    The mapping retains legacy RWX permissions for existing fixture hooks. That
    does not authorize treating patched code or supplied call results as native.
    """
    data = image_bytes()
    sections = list(_sections(data))
    uc.mem_map(IMAGE_BASE, IMAGE_SIZE)
    uc.mem_write(IMAGE_BASE, data[:0x1000])
    for rva, raw_ptr, raw_size, _, _ in sections:
        if raw_size:
            uc.mem_write(IMAGE_BASE + rva, data[raw_ptr:raw_ptr + raw_size])


def run_checked(uc: Uc, begin: int, end: int | tuple[int, ...], *,
                count: int = 5_000_000, timeout_us: int = 10_000_000,
                required_addresses=(), context: dict | None = None) -> int:
    """Execute to a declared return/region boundary or fail with a short trace.

    Boundaries are reached BEFORE executing their instruction. Existing hooks may
    remain, but stopping anywhere else fails. No faults are swallowed. Unicorn's
    count/time limits stop emulation normally, so absence of UcError is not proof
    of completion (uc.c:987, unicorn.h uc_emu_start). This function owns exits for
    this run; custom ctl_set_exits are disabled in favor of these explicit ends.

    Failures carry JSON-compatible diagnostics; set VERA20K_NATIVE_FAILURE_DIR
    to also save them. context may identify the case and supplied fixture inputs.
    A reached instruction budget is an observation, not proof that an external
    hook did not stop at that same instruction.
    """
    ends = (end,) if isinstance(end, int) else tuple(end)
    if not ends or begin in ends or count <= 0 or timeout_us <= 0:
        raise ValueError("Use distinct entry/endpoints and positive instruction/time limits")
    required = set(required_addresses)
    if required.intersection(ends):
        raise ValueError("Required instruction addresses must precede the stop boundary")
    if context is not None and not isinstance(context, dict):
        raise TypeError("Native diagnostic context must be a JSON object")
    # Freeze caller inputs before hooks execute, and fail invalid context before
    # running anything. This does not change the successful result schema.
    diagnostic_context = json.loads(_canonical(context or {}))
    initial = _machine_state(uc)
    visited = set()
    trail = deque(maxlen=16)
    observed = 0

    def observe(_uc, address, _size, _data):
        nonlocal observed
        trail.append(address)
        if address in required:
            visited.add(address)
        if address in ends:
            _uc.emu_stop()
        else:
            observed += 1

    uc.ctl_exits_enabled(False)
    hook = uc.hook_add(UC_HOOK_CODE, observe)
    try:
        fault = None
        try:
            uc.emu_start(begin, ends[0], timeout=timeout_us, count=count)
        except UcError as error:
            fault = error
        # Query before another emulation or diagnostic callback can replace it.
        timed_out = bool(uc.query(UC_QUERY_TIMEOUT))
        pc = uc.reg_read(UC_X86_REG_EIP)
        missing = required - visited
        if fault or timed_out or pc not in ends or missing:
            if fault:
                reason = "fault"
                message = f"Native execution 0x{begin:08X} faulted: {fault}"
            elif timed_out or pc not in ends:
                reason = ("timeout" if timed_out else
                          "instruction_limit_reached" if observed >= count else "early_stop")
                message = (f"Incomplete execution from 0x{begin:08X}: stopped at 0x{pc:08X}; "
                           f"expected {', '.join(f'0x{x:08X}' for x in ends)}")
            else:
                reason = "required_addresses_missing"
                message = f"Required native instruction addresses not reached: {sorted(hex(x) for x in missing)}"
            trace = ", ".join(f"0x{x:08X}" for x in trail)
            diagnostics = {
                "schema": "vera20k.native-execution-failure.v1", "reason": reason,
                "entry": begin, "expected_endpoints": list(ends), "timed_out": timed_out,
                "instruction_limit": count, "timeout_us": timeout_us,
                "observed_instructions": observed,
                "required_addresses": sorted(required), "missing_required_addresses": sorted(missing),
                "trace": list(trail), "initial": initial, "final": _machine_state(uc),
                "context": diagnostic_context,
                "fault": {"errno": fault.errno, "message": str(fault)} if fault else None,
                "unicorn_binding": unicorn.__version__, "unicorn_core": list(unicorn.uc_version()),
                # run_checked also accepts synthetic machines and cannot attest
                # their image identity just because this runner knows the pin.
                "expected_native_sha256": NATIVE_SHA256,
            }
            raise NativeExecutionError(
                f"{message}; reason={reason}, timeout={timed_out}, "
                f"observed={observed}/{count}; trace: {trace}", diagnostics) from fault
        return pc
    finally:
        uc.hook_del(hook)


def call(func: int, *, ecx=None, edx=None, stack_args=None, writes=None,
         dumps=None, capture_st0=False, fpcw=NATIVE_FPCW,
         timeout_instr=5_000_000, timeout_us=10_000_000, required_addresses=(),
         context: dict | None = None) -> dict:
    """Run one function in fresh state. Results preserve the legacy harness schema.

    ECX/EDX and stack arguments are explicit calling-convention inputs. Writes
    supply fixture data; executable-section writes are rejected. ST0 capture
    stores binary64 through a six-byte FSTP return stub outside native memory;
    it is a declared observation conversion, not full x87 80-bit capture.
    """
    uc = Uc(UC_ARCH_X86, UC_MODE_32)
    sections = list(_sections(image_bytes()))
    if not any(flags & 0x20000000 and IMAGE_BASE + rva <= func < IMAGE_BASE + rva + raw
               for rva, _, raw, _, flags in sections):
        raise OracleError("call() entry must be an original native executable-section address")
    load_image(uc)
    uc.mem_map(STACK_BASE, STACK_SIZE)
    uc.mem_map(SCRATCH, SCRATCH_SIZE)
    uc.mem_map(RET_MAGIC, 0x1000)
    executable = [(IMAGE_BASE + rva, IMAGE_BASE + rva + max(raw, size))
                  for rva, _, raw, size, flags in sections if flags & 0x20000000]
    for address, blob in (writes or {}).items():
        if any(address < high and address + len(blob) > low for low, high in executable):
            raise OracleError("call() fixture writes cannot replace native executable instructions")
        uc.mem_write(address, blob)
    stop_at = RET_MAGIC
    st0_slot = RET_MAGIC + 0x100
    if capture_st0:
        uc.mem_write(RET_MAGIC, b"\xdd\x1d" + struct.pack("<I", st0_slot))
        stop_at += 6
    sp = STACK_BASE + STACK_SIZE - 0x1000
    for value in reversed(stack_args or []):
        sp -= 4
        uc.mem_write(sp, struct.pack("<I", value))
    sp -= 4
    uc.mem_write(sp, struct.pack("<I", RET_MAGIC))
    uc.reg_write(UC_X86_REG_ESP, sp)
    for register, value in [(UC_X86_REG_FPCW, fpcw), (UC_X86_REG_ECX, ecx), (UC_X86_REG_EDX, edx)]:
        if value is not None:
            uc.reg_write(register, value)
    required = set(required_addresses)
    if capture_st0:
        required.add(RET_MAGIC)
    detail = dict(context or {})
    if "call" in detail:
        raise ValueError("The diagnostic context key 'call' is reserved for calling-convention inputs")
    detail["call"] = {
        "function": func, "ecx": ecx, "edx": edx, "stack_args": list(stack_args or []),
        "fpcw": fpcw, "capture_st0": capture_st0,
        "writes": [{"address": address, "bytes": len(blob)} for address, blob in (writes or {}).items()],
    }
    run_checked(uc, func, stop_at, count=timeout_instr, timeout_us=timeout_us,
                required_addresses=required, context=detail)
    result = {"eax": uc.reg_read(UC_X86_REG_EAX) & 0xFFFFFFFF, "dumps": {}}
    for name, (address, length) in (dumps or {}).items():
        result["dumps"][name] = bytes(uc.mem_read(address, length)).hex()
    if capture_st0:
        raw = bytes(uc.mem_read(st0_slot, 8))
        result.update(st0_bits=struct.unpack("<Q", raw)[0], st0=struct.unpack("<d", raw)[0])
    return result


def provenance(*, scope: str, assumptions: list[str], substitutions: list[str],
               entry_points: dict[str, int]) -> dict:
    """Record identity and claims separately from legacy vector payloads."""
    if not scope.strip() or not assumptions or not entry_points:
        raise ValueError("Declare scope, runtime assumptions, and native entry points")
    return {
        "schema_version": 1, "native_sha256": hashlib.sha256(image_bytes()).hexdigest(),
        "unicorn_binding": unicorn.__version__, "unicorn_core": list(unicorn.uc_version()),
        "scope": scope, "assumptions": assumptions, "substitutions": substitutions,
        "entry_points": {name: f"0x{value:08X}" for name, value in entry_points.items()},
    }


def _canonical(data) -> bytes:
    return json.dumps(data, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def first_difference(expected, actual, path="$", limit=180) -> str | None:
    """Locate the first mismatch without dumping whole model states."""
    if type(expected) is not type(actual):
        return f"{path}: expected {type(expected).__name__}, got {type(actual).__name__}"
    if isinstance(expected, dict):
        if expected.keys() != actual.keys():
            return f"{path}: missing keys {sorted(expected.keys() - actual.keys())}, extra keys {sorted(actual.keys() - expected.keys())}"
        for key in expected:
            if difference := first_difference(expected[key], actual[key], f"{path}.{key}"):
                return difference
    elif isinstance(expected, list):
        if len(expected) != len(actual):
            return f"{path}: expected {len(expected)} entries, got {len(actual)}"
        for index, (left, right) in enumerate(zip(expected, actual)):
            if difference := first_difference(left, right, f"{path}[{index}]"):
                return difference
    elif isinstance(expected, float) and struct.pack("<d", expected) != struct.pack("<d", actual):
        return f"{path}: expected {expected!r}, got {actual!r} (binary64 differs)"
    elif isinstance(expected, str) and expected != actual and max(len(expected), len(actual)) > limit:
        offset = next((i for i, (left, right) in enumerate(zip(expected, actual)) if left != right),
                      min(len(expected), len(actual)))
        start = max(0, offset - 24)
        return (f"{path}: first differing character {offset}; "
                f"expected {expected[start:offset + 40]!r}, got {actual[start:offset + 40]!r}")
    elif expected != actual:
        return f"{path}: expected {repr(expected)[:limit]}, got {repr(actual)[:limit]}"
    return None


def finish_vectors(data, default_path: Path, *, provenance: dict, argv=None,
                   source_paths: dict[str, Path] | None = None) -> None:
    """Default: check without writing. --write deliberately replaces the reference.

    Existing payloads retain their Rust-facing schema. A .meta.json sidecar records
    provenance and the canonical payload hash. Old files without metadata can be
    compared, but their historical provenance is explicitly unknown. Optional
    source_paths captures UTF-8/LF source identity before invoking the lazy
    generator and rejects drift immediately before comparison or publication.
    """
    parser = argparse.ArgumentParser(description="Compare native outputs with recorded reference data")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="compare only (default)")
    mode.add_argument("--write", action="store_true", help="explicitly write outputs and provenance")
    parser.add_argument("--output", type=Path, default=default_path)
    args = parser.parse_args(argv)
    def source_identity():
        return {name: hashlib.sha256(path.read_text(encoding="utf-8").encode("utf-8")).hexdigest()
                for name, path in (source_paths or {}).items()}

    sources = source_identity()
    data = data() if callable(data) else data
    provenance = provenance() if callable(provenance) else provenance
    if source_paths is not None:
        if "source_normalized_lf_sha256" in provenance:
            raise OracleError("Source provenance must be supplied through source_paths")
        provenance = dict(provenance, source_normalized_lf_sha256=sources)
    # Normalize tuples before comparisons; reject NaN/Infinity in either workflow.
    normalized = json.loads(_canonical(data))
    metadata = dict(provenance, payload_sha256=hashlib.sha256(_canonical(normalized)).hexdigest())
    payload_text = json.dumps(normalized, indent=2, allow_nan=False) + "\n"
    metadata_text = json.dumps(metadata, indent=2, allow_nan=False) + "\n"
    target = args.output
    sidecar = target.with_suffix(".meta.json")
    if difference := first_difference(sources, source_identity()):
        raise OracleError(f"Source changed during native generation: {difference}")
    if args.write:
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(payload_text, encoding="utf-8")
        sidecar.write_text(metadata_text, encoding="utf-8")
        print(f"WROTE {target} and {sidecar}; review before accepting changed native references")
        return
    if not target.is_file():
        raise OracleError(f"Reference missing: {target}; use --write to deliberately create it")
    expected = json.loads(target.read_text(encoding="utf-8"))
    if difference := first_difference(expected, normalized):
        raise OracleError(f"Reference mismatch in {target}: {difference}")
    if sidecar.is_file():
        expected_metadata = json.loads(sidecar.read_text(encoding="utf-8"))
        if difference := first_difference(expected_metadata, metadata):
            raise OracleError(f"Provenance mismatch in {sidecar}: {difference}")
    else:
        print("NOTE: legacy reference has no provenance sidecar; historical environment is unknown")
    print(f"PASS {target}: native outputs match; no files written")
