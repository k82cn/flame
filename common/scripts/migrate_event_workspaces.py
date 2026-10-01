#!/usr/bin/env python3
# Copyright 2026 The Flame Authors.
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#     http://www.apache.org/licenses/LICENSE-2.0
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
"""Copy offline legacy events to a new workspace event root.

Run OLD_EVENTS NEW_EVENTS [--apply] after stopping all writers and taking a
snapshot. The default dry run validates everything without writing. After
--apply succeeds, configure event storage to NEW_EVENTS. OLD_EVENTS is retained.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import struct
import tempfile

from migrate_filesystem_workspaces import name

# bincode 2: fixed integers, big endian; Option<u64> has a one-byte tag.
RECORD = struct.Struct('>BQqiQQq')
FILES = {'events.json', 'events.csv', 'events.dat', 'event_messages.dat'}


def migrate(source, destination, apply=False):
    source, destination = source.resolve(), destination.absolute()
    if not source.is_dir() or destination.exists() or destination.is_symlink():
        raise ValueError('source must exist and destination must not exist')
    if source == destination or source in destination.parents:
        raise ValueError('destination must be outside source')
    sessions = {}
    for directory in sorted(source.iterdir()):
        name(directory.name)
        if directory.is_symlink() or not directory.is_dir():
            raise ValueError(f'expected session directory: {directory}')
        entries = list(directory.iterdir())
        if {entry.name for entry in entries} != FILES or any(
                entry.is_symlink() or not entry.is_file() for entry in entries):
            raise ValueError(f'unexpected legacy event files: {directory}')
        data = (directory / 'events.dat').read_bytes()
        messages = (directory / 'event_messages.dat').read_bytes()
        metadata = (directory / 'events.json').read_text()
        size = json.loads(metadata)['size'] if metadata else 0
        if (data and size != RECORD.size) or len(data) % RECORD.size:
            raise ValueError(f'invalid event record size: {directory}')
        owners = {}
        for line in (directory / 'events.csv').read_text().splitlines():
            identity, owner = map(int, line.split(','))
            if identity in owners:
                raise ValueError(f'duplicate event index: {directory}')
            owners[identity] = owner
        records = []
        for index in range(len(data) // RECORD.size):
            tag, identity, owner, code, start, end, timestamp = RECORD.unpack_from(data, index * RECORD.size)
            if tag != 1 or identity != index + 1 or owners.pop(identity, None) != owner or owner < 0:
                raise ValueError(f'invalid event identity/owner: {directory}')
            if not 0 <= start <= end <= len(messages):
                raise ValueError(f'invalid event message range: {directory}')
            records.append({'task': None if owner == 0 else str(owner), 'code': code,
                            'message': messages[start:end].decode('utf-8'), 'creation_time': timestamp})
        if owners:
            raise ValueError(f'event index references missing records: {directory}')
        sessions[directory.name] = records
    counts = {'sessions': len(sessions), 'events': sum(map(len, sessions.values()))}
    if not apply:
        return counts
    destination.parent.mkdir(parents=True, exist_ok=True)
    stage = Path(tempfile.mkdtemp(prefix='.flame-events-', dir=destination.parent))
    try:
        for session, records in sessions.items():
            target = stage / 'default' / session
            target.mkdir(parents=True)
            with (target / 'events.jsonl').open('w') as output:
                for record in records:
                    output.write(json.dumps(record) + '\n')
                output.flush()
                os.fsync(output.fileno())
        for root, _, _ in os.walk(stage, topdown=False):
            descriptor = os.open(root, os.O_RDONLY)
            try:
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
        if destination.exists() or destination.is_symlink():
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
    args = parser.parse_args()
    print(json.dumps(migrate(args.source, args.destination, args.apply), sort_keys=True))
