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

"""Benchmark the object cache with the weight-sync pattern of an RL training loop.

Run from the benchmarks directory with a configured Flame client:
    python3 cache.py

Every step publishes a new version of a model as SHARDS objects of SHARD_MB each (what a
trainer does after an update), then READERS executors each load every shard of that
version (what rollout workers do before generating), and the previous version is deleted.
Each executor checks the SHA-256 of every shard it reads, so wrong or stale data fails the
benchmark. Set SHARDS=12 SHARD_MB=512 for a 6 GB checkpoint, the Search-R1 demo's size.

    SHARDS=4 SHARD_MB=256 READERS=4 STEPS=3 python3 cache.py
"""

import hashlib
import os
import statistics
import time
import uuid

from flamepy import app
from flamepy.core import ObjectRef, delete_objects, get_object, put_object

SHARDS = int(os.environ.get("SHARDS", "4"))
SHARD_MB = int(os.environ.get("SHARD_MB", "256"))
READERS = int(os.environ.get("READERS", "4"))
STEPS = int(os.environ.get("STEPS", "3"))
APP_NAME = f"flamepy-cache-benchmark-{uuid.uuid4().hex[:8]}"
app.init(APP_NAME)

MIB = 1024 * 1024
BLOCK = os.urandom(MIB)
# The SDK keeps objects it has read, so an executor can hold a whole version plus the shard
# being read; the default 2 GiB would be OOM-killed for a multi-GiB checkpoint.
READER_MEM_GIB = (SHARDS * SHARD_MB * 3 // 2 + 2 * SHARD_MB) // 1024 + 1


@app.service(resreq=f"cpu=1,mem={READER_MEM_GIB}g", warmup=READERS)
def load(shards: list[dict]) -> dict:
    """Read every shard of one version, as a rollout worker does, and return its digests."""
    started = time.perf_counter()
    digests = []
    for shard in shards:
        data = get_object(ObjectRef(**shard))
        digests.append(hashlib.sha256(data).hexdigest())
    return {"seconds": time.perf_counter() - started, "digests": digests}


def make_shard(step: int, index: int) -> bytes:
    """A shard unique to (step, index): a distinct header, then the repeated random block."""
    header = f"{APP_NAME}/{step}/{index}".encode().ljust(MIB, b"\0")
    return header + BLOCK * (SHARD_MB - 1)


def publish(step: int) -> tuple[list[dict], list[str], float]:
    prefix = f"{APP_NAME}/step-{step}"
    refs = []
    shards = [make_shard(step, index) for index in range(SHARDS)]
    digests = [hashlib.sha256(shard).hexdigest() for shard in shards]
    started = time.perf_counter()
    for shard in shards:
        ref = put_object(prefix, shard)
        refs.append({"endpoint": ref.endpoint, "key": ref.key, "version": ref.version})
    return refs, digests, time.perf_counter() - started


def main() -> None:
    total_mib = SHARDS * SHARD_MB
    rows = []
    previous = None
    service = app.remote(load)
    try:
        for step in range(1, STEPS + 1):
            refs, digests, publish_s = publish(step)
            started = time.perf_counter()
            futures = [service(refs) for _ in range(READERS)]
            results = app.get(futures)
            fanout_s = time.perf_counter() - started
            for result in results:
                if result["digests"] != digests:
                    raise RuntimeError(f"step {step}: a reader got wrong or stale shard data")
            loads = [result["seconds"] for result in results]
            rows.append((step, publish_s, loads, fanout_s))
            if previous is not None:
                delete_objects(previous)
            previous = f"{APP_NAME}/step-{step}"
    finally:
        service.close()
        if previous is not None:
            delete_objects(previous)
        app.destroy()

    print("## flamepy object cache benchmark", flush=True)
    print(f"Checkpoint: {SHARDS} shards x {SHARD_MB} MiB = {total_mib} MiB per version; {READERS} readers; {STEPS} steps; every shard checked by SHA-256.\n", flush=True)
    print("| Step | Publish s | Publish MiB/s | Load s (min/p50/max) | Fan-out s | Aggregate read MiB/s |", flush=True)
    print("| ---: | ---: | ---: | ---: | ---: | ---: |", flush=True)
    for step, publish_s, loads, fanout_s in rows:
        load_range = "/".join(f"{value:.1f}" for value in (min(loads), statistics.median(loads), max(loads)))
        print(f"| {step} | {publish_s:.1f} | {total_mib / publish_s:.0f} | {load_range} | {fanout_s:.1f} | {total_mib * len(loads) / fanout_s:.0f} |", flush=True)


if __name__ == "__main__":
    main()
