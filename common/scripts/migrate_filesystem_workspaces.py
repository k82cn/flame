#!/usr/bin/env python3
# Copyright 2026 The Flame Authors.
# Licensed under the Apache License, Version 2.0.
"""Copy an offline legacy filesystem store into a new workspace store.

Usage: python3 migrate_filesystem_workspaces.py OLD_ROOT NEW_ROOT [--apply]
Without --apply, validate and report only. NEW_ROOT must not exist. Stop all
writers and snapshot OLD_ROOT first. After success, configure filesystem://NEW_ROOT.
The original store is retained for rollback; never run old and new writers together.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import struct
import tempfile
import time
import uuid
import zlib
from urllib.parse import urlsplit, urlunsplit

RECORD = struct.Struct('<QIIBqqQQQQQQ')


def name(value):
    if (not isinstance(value, str) or len(value) > 253 or '..' in value
            or not re.fullmatch(r'[A-Za-z0-9_][A-Za-z0-9_.-]*', value)):
        raise ValueError(f'unsafe resource name: {value!r}')
    return value


def records(root, kind):
    directory = root / kind
    result = {}
    if directory.exists():
        for entry in sorted(directory.iterdir()):
            name(entry.name)
            if not entry.is_dir():
                raise ValueError(f'expected resource directory: {entry}')
            result[entry.name] = json.loads((entry / 'metadata').read_text())
    return result


def tasks(path):
    """Validate legacy fixed-width records and payload ranges before any writes."""
    source = path / 'tasks.bin'
    if not source.exists():
        return []
    data = source.read_bytes()
    if len(data) % RECORD.size:
        raise ValueError(f'truncated task metadata: {source}')
    output = []
    for index in range(len(data) // RECORD.size):
        raw = data[index * RECORD.size:(index + 1) * RECORD.size]
        fields = RECORD.unpack(raw)
        if fields[0] != index + 1 or fields[2] != zlib.crc32(raw[:12] + raw[16:65]):
            raise ValueError(f'invalid task number/checksum: {source}, record {index + 1}')
        for filename, offset, size in [('inputs.bin', fields[6], fields[7]),
                                      ('outputs.bin', fields[8], fields[9]),
                                      ('affinity.bin', fields[10], fields[11])]:
            payload = path / filename
            if size and (not payload.is_file() or offset + size > payload.stat().st_size):
                raise ValueError(f'invalid payload range: {payload}, task {index + 1}')
        output.append(raw)
    return output


def migrate(source, destination, apply=False):
    source, destination = source.resolve(), destination.absolute()
    if not source.is_dir() or destination.exists() or destination.is_symlink():
        raise ValueError('source must exist and destination must not exist')
    if source == destination or source in destination.parents:
        raise ValueError('destination must be outside source')
    if (source / 'workspaces').exists():
        raise ValueError('source already contains workspaces')
    for root, directories, files in os.walk(source):
        for component in directories + files:
            if (Path(root) / component).is_symlink():
                raise ValueError(f'symlink in source: {Path(root) / component}')
    if any(entry.name not in {'applications', 'sessions', 'nodes', 'executors'} for entry in source.iterdir()):
        raise ValueError('unexpected entries in resource store; inventory them before migrating')
    apps = records(source, 'applications')
    sessions = records(source, 'sessions')
    nodes = records(source, 'nodes')
    executors = records(source, 'executors')
    task_records = {}
    for key, meta in apps.items():
        if meta.get('name') != key:
            raise ValueError(f'application name/path mismatch: {key}')
    for key, meta in sessions.items():
        if meta.get('id') != key or meta.get('application') not in apps:
            raise ValueError(f'invalid session name/application: {key}')
        task_records[key] = tasks(source / 'sessions' / key)
    for key, meta in nodes.items():
        if meta.get('name') != key:
            raise ValueError(f'node name/path mismatch: {key}')
    for key, meta in executors.items():
        session, task = meta.get('ssn_id'), meta.get('task_id')
        if meta.get('id') != key or meta.get('node') not in nodes or meta.get('application') not in apps:
            raise ValueError(f'invalid executor references: {key}')
        if session is not None and (session not in sessions or sessions[session]['application'] != meta['application']):
            raise ValueError(f'invalid executor session: {key}')
        if task is not None and (session is None or not isinstance(task, int) or not 1 <= task <= len(task_records[session])):
            raise ValueError(f'invalid executor task: {key}')
    counts = {'applications': len(apps), 'sessions': len(sessions), 'nodes': len(nodes),
              'executors': len(executors), 'tasks': sum(map(len, task_records.values()))}
    if not apply:
        return counts
    destination.parent.mkdir(parents=True, exist_ok=True)
    stage = Path(tempfile.mkdtemp(prefix='.flame-workspaces-', dir=destination.parent))
    try:
        workspace = stage / 'workspaces' / 'default'
        workspace.mkdir(parents=True)
        (workspace / 'workspace.json').write_text(json.dumps({'name': 'default', 'create_at': int(time.time() * 1000)}))
        for kind, resources in [('applications', apps), ('sessions', sessions), ('executors', executors), ('nodes', nodes)]:
            target = stage / kind if kind == 'nodes' else workspace / kind
            target.mkdir()
            for key, meta in resources.items():
                shutil.copytree(source / kind / key, target / key)
                if kind != 'nodes':
                    meta['workspace'] = 'default'
                if kind == 'applications' and meta.get('url'):
                    parsed = urlsplit(meta['url'])
                    if parsed.scheme in {'grpc', 'grpcs', 'grpc+tls', 'grpcs-proxy'}:
                        meta['url'] = urlunsplit(parsed._replace(path='/default' + parsed.path))
                meta['name'] = key
                old_id = meta.get('id')
                try:
                    parsed = uuid.UUID(old_id) if kind == 'executors' else None
                except (ValueError, TypeError, AttributeError):
                    parsed = None
                meta['id'] = str(parsed or uuid.uuid4())
                if kind == 'executors':
                    meta['session'] = meta.pop('ssn_id', None)
                    task = meta.pop('task_id', None)
                    meta['task'] = None if task is None else str(task)
                (target / key / 'metadata').write_text(json.dumps(meta))
                if kind == 'sessions' and (target / key / 'tasks.bin').exists():
                    with (target / key / 'tasks.bin').open('wb') as output:
                        for raw in task_records[key]:
                            updated = raw[:8] + uuid.uuid4().bytes + raw[8:]
                            checksum = zlib.crc32(updated[:28] + updated[32:81])
                            output.write(updated[:28] + struct.pack('<I', checksum) + updated[32:])
        (stage / 'workspace-migration.json').write_text(json.dumps({'version': 1, 'source': str(source), 'counts': counts}))
        # Flush all rewritten files and directory entries before publishing the root.
        for root, _, files in os.walk(stage, topdown=False):
            for filename in files:
                with (Path(root) / filename).open('rb') as stream:
                    os.fsync(stream.fileno())
            descriptor = os.open(root, os.O_RDONLY)
            try:
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
        if destination.exists():
            raise ValueError('destination appeared during migration')
        stage.rename(destination)
        descriptor = os.open(destination.parent, os.O_RDONLY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    finally:
        if stage.exists():
            shutil.rmtree(stage)
    return counts


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('destination', type=Path)
    parser.add_argument('--apply', action='store_true')
    arguments = parser.parse_args()
    print(json.dumps(migrate(arguments.source, arguments.destination, arguments.apply), sort_keys=True))
