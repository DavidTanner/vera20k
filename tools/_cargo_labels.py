"""Explicit saved-build retirement under cargo_run's shared owner lock.

This never follows manifests back into compiler caches or external evidence.
Retirement currently requires POSIX lsof; unavailable/failed inspection (including
Windows) fails closed. Inspection covers processes visible to the invoking user.
As with cache retention, the lock excludes cooperating writers, not arbitrary
programs launched outside cargo_run: stop consumers before retiring their labels.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import uuid

from tools._cargo_cache import _allocated, _directory_identity, _identity, _plain, _regular


def _idle(paths: list[Path]):
    if sys.platform not in {'darwin', 'linux'}:
        raise ValueError('Saved-build retirement requires Linux/macOS lsof process inspection')
    executable = shutil.which('lsof')
    if not executable:
        raise ValueError('Saved-build retirement requires lsof; no files removed')
    for offset in range(0, len(paths), 64):
        result = subprocess.run([executable, '-nP', '-Fpn', '--',
                                 *(str(path) for path in paths[offset:offset + 64])],
                                capture_output=True, text=True, timeout=30)
        if result.stdout.strip():
            raise ValueError('Selected saved-build files are in use: ' + result.stdout.strip())
        if result.returncode != 1 or result.stderr.strip():
            raise ValueError('Cannot establish idle saved-build files: ' +
                             (result.stderr.strip() or f'lsof exit {result.returncode}'))


def _durable_receipt(path: Path, value: dict):
    """Persist the complete plan before unlinking; atomic updates record progress."""
    _plain(path.parent)
    pending = None
    try:
        with tempfile.NamedTemporaryFile(mode='w', dir=path.parent, delete=False,
                                         prefix='.retirement-', encoding='utf-8') as output:
            pending = Path(output.name)
            json.dump(value, output, indent=2)
            output.write('\n')
            output.flush()
            os.fsync(output.fileno())
        pending.replace(path)
        descriptor = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    finally:
        if pending is not None:
            pending.unlink(missing_ok=True)


def _inventory(store: Path, label: str):
    from tools.cargo_run import _label_path, preserved_artifact, preserved_manifest
    manifest = _label_path(store.parent.resolve(), f'{store.name}/artifacts/{label}/manifest.json')
    verified = {manifest: _identity(manifest)}
    directory, _, artifacts = preserved_manifest(store, label)
    expected = {directory / 'manifest.json'}
    paths = []
    for artifact in artifacts:
        path = _label_path(directory, artifact['file'])
        before = _identity(path)
        preserved_artifact(directory, artifact)
        if _identity(path) != before:
            raise ValueError(f'Saved artifact changed while hashing: {path}')
        verified[path] = before
        if path in expected:
            raise ValueError(f'Duplicate/overlapping saved artifact: {path}')
        expected.add(path)
        paths.append(path)
    allowed_dirs = {directory}
    for path in expected:
        allowed_dirs.update(parent for parent in path.parents if parent.is_relative_to(directory))
    directories = {path: _directory_identity(path) for path in allowed_dirs}
    # No recursive deletion: even unexpected empty directories prevent retirement.
    actual = set()
    for parent, names, filenames in os.walk(directory, followlinks=False):
        for name in names:
            path = Path(parent) / name
            if path not in allowed_dirs:
                raise ValueError(f'Unexpected saved-build directory: {path}')
            _directory_identity(path)
        actual.update(Path(parent) / name for name in filenames)
    if actual != expected:
        raise ValueError(f'Unexpected/missing saved-build files: {directory}: {actual ^ expected}')
    identities = {}
    allocated = 0
    for path in expected:
        info = _regular(path)
        if info.st_nlink != 1:
            raise ValueError(f'Shared hardlink cannot be retired: {path}')
        identities[path] = _identity(path)
        if identities[path] != verified[path]:
            raise ValueError(f'Saved-build file changed during inventory: {path}')
        allocated += _allocated(info)
    raw = manifest.read_bytes()
    if _identity(manifest) != verified[manifest]:
        raise ValueError(f'Saved manifest changed during inventory: {manifest}')
    return {
        'label': label, 'directory': directory, 'paths': paths,
        'identities': identities, 'directories': directories,
        'manifest_text': raw.decode('utf-8'),
        'manifest_sha256': hashlib.sha256(raw).hexdigest(),
        'allocated_bytes': allocated,
    }


def _unchanged(item):
    for path, identity in item['directories'].items():
        if _directory_identity(path) != identity:
            raise ValueError(f'Saved-build directory changed during retirement: {path}')
    for path, identity in item['identities'].items():
        if _identity(path) != identity:
            raise ValueError(f'Saved-build file changed during retirement: {path}')


def retire(root: Path, labels: list[str], timeout: float, dry_run: bool = False) -> dict:
    """Retire exact named labels, preserving original manifests and byte identities.

    All labels preflight before any deletion. Empty/glob/duplicate selections fail.
    Dry runs return a plan without writing a receipt or changing source artifacts.
    A durable receipt remains after success or partial failure. Recovery after a
    process/system crash can reconcile planned paths with disk; completed unlinks
    are recorded after each file. This is not a transactional filesystem operation.
    """
    from tools.cargo_run import build_lock, build_processes, build_store, validate_label
    if not labels or len(set(labels)) != len(labels):
        raise ValueError('Supply one or more unique exact saved-build labels')
    for label in labels:
        validate_label(label)
    store, _ = build_store(root)
    receipt = {
        'schema': 1, 'operation': 'retire-saved-builds', 'state': 'blocked',
        'started_unix': time.time(), 'dry_run': dry_run, 'labels': list(labels),
        'builds': [], 'removed_files': [], 'removed_labels': [], 'errors': [],
        'removed_allocated_bytes': 0, 'projected_removed_allocated_bytes': 0,
        'process_inspection_scope': 'Linux/macOS lsof; processes visible to invoking user',
    }
    receipt_path = None
    with build_lock(store / 'cargo.lock', timeout):
        try:
            items = [_inventory(store, label) for label in labels]
            paths = [path for item in items for path in item['paths']]
            _idle(paths)
            for item in items:
                _unchanged(item)
            if build_processes():
                raise ValueError('Unwrapped Cargo/rustc became active; no deletion')
            receipt['builds'] = [
                {key: item[key] for key in ('label', 'manifest_text', 'manifest_sha256', 'allocated_bytes')}
                for item in items]
            receipt['projected_removed_allocated_bytes'] = sum(item['allocated_bytes'] for item in items)
            receipt['selected_files'] = [str(path) for item in items for path in item['identities']]
            receipt['state'] = 'planned'
            if dry_run:
                return receipt
            parent = store / 'label-retirements'
            parent.mkdir(exist_ok=True)
            _plain(parent)
            descriptor = os.open(store, os.O_RDONLY)
            try:
                os.fsync(descriptor)  # Persist new receipt directory before any unlink.
            finally:
                os.close(descriptor)
            receipt_path = parent / f'{time.time_ns()}-{uuid.uuid4().hex}.json'
            receipt['receipt_path'] = str(receipt_path)
            receipt['free_before'] = shutil.disk_usage(store).free
            _durable_receipt(receipt_path, receipt)
            for item in items:
                _unchanged(item)
                _idle(item['paths'])
                if build_processes():
                    raise ValueError('Unwrapped Cargo/rustc became active; stopping retirement')
                # Manifest last: an interrupted label retains its lookup metadata.
                for path in [*item['paths'], item['directory'] / 'manifest.json']:
                    if _identity(path) != item['identities'][path]:
                        raise ValueError(f'Saved-build file changed before unlink: {path}')
                    allocated = _allocated(path.stat())
                    path.unlink()
                    receipt['removed_files'].append(str(path))
                    receipt['removed_allocated_bytes'] += allocated
                    _durable_receipt(receipt_path, receipt)
                for directory in sorted(item['directories'], key=lambda path: len(path.parts), reverse=True):
                    directory.rmdir()
                receipt['removed_labels'].append(item['label'])
                _durable_receipt(receipt_path, receipt)
            receipt['state'] = 'retired'
        except (OSError, ValueError, UnicodeError, subprocess.SubprocessError) as error:
            receipt['state'] = 'partial' if receipt['removed_files'] else 'blocked'
            receipt['errors'].append(str(error))
        receipt['finished_unix'] = time.time()
        if receipt_path:
            try:
                receipt['free_after'] = shutil.disk_usage(store).free
                receipt['observed_free_delta_bytes'] = (
                    receipt['free_after'] - receipt['free_before']
                    if 'free_before' in receipt else None)
            except OSError as error:
                receipt['state'] = 'partial' if receipt['removed_files'] else 'blocked'
                receipt['free_after'] = None
                receipt['observed_free_delta_bytes'] = None
                receipt['errors'].append(f'Cannot measure final free space: {error}')
            try:
                _durable_receipt(receipt_path, receipt)
            except OSError as error:
                receipt['state'] = 'partial' if receipt['removed_files'] else 'blocked'
                receipt['errors'].append(f'Cannot persist final retirement receipt: {error}')
    return receipt
