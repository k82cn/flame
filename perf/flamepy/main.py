"""
Copyright 2025 The Flame Authors.
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at
    http://www.apache.org/licenses/LICENSE-2.0
Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
"""

import argparse
import os
import time
from concurrent.futures import Future, wait
from dataclasses import dataclass
from typing import List

from flamepy.core import (
    ApplicationAttributes,
    Session,
    create_session,
    get_application,
    register_application,
    unregister_application,
)

NUM_SESSIONS = 10
TASKS_PER_SESSION = 1000
TOTAL_TASKS = NUM_SESSIONS * TASKS_PER_SESSION
TIMEOUT_SECS = 600

BENCHMARK_APP = "flamepy-bench"


@dataclass
class BenchmarkResult:
    succeeded: int
    failed: int
    duration_secs: float

    @property
    def throughput(self) -> float:
        return self.succeeded / self.duration_secs if self.duration_secs > 0 else 0


def setup_benchmark_app() -> bool:
    if get_application(BENCHMARK_APP) is not None:
        return False

    working_dir = os.path.dirname(os.path.abspath(__file__))
    register_application(
        BENCHMARK_APP,
        ApplicationAttributes(
            command="python3",
            arguments=["service.py"],
            working_directory=working_dir,
        ),
    )
    return True


def teardown_benchmark_app(registered: bool) -> None:
    if registered:
        unregister_application(BENCHMARK_APP)


def run_benchmark(
    num_sessions: int = NUM_SESSIONS,
    tasks_per_session: int = TASKS_PER_SESSION,
) -> BenchmarkResult:
    total_tasks = num_sessions * tasks_per_session

    print("\n" + "=" * 60)
    print(
        f"BENCHMARK: {num_sessions} sessions × {tasks_per_session} tasks = {total_tasks} total"
    )
    print("=" * 60 + "\n")

    start = time.perf_counter()

    sessions: List[Session] = []
    all_futures: List[Future] = []

    for _ in range(num_sessions):
        session = create_session(BENCHMARK_APP)
        sessions.append(session)
        for _ in range(tasks_per_session):
            future = session.run(b"benchmark")
            all_futures.append(future)

    print(f"Submitted {len(all_futures)} tasks, waiting for completion...")

    wait(all_futures)

    duration = time.perf_counter() - start

    succeeded = 0
    failed = 0
    for future in all_futures:
        try:
            future.result()
            succeeded += 1
        except Exception:
            failed += 1

    for session in sessions:
        session.close()

    return BenchmarkResult(
        succeeded=succeeded,
        failed=failed,
        duration_secs=duration,
    )


def print_results(result: BenchmarkResult, total_tasks: int) -> None:
    print("\n" + "=" * 60)
    print("BENCHMARK RESULTS")
    print("=" * 60)
    print(f"Duration:        {result.duration_secs:.2f}s")
    print(f"Succeeded:       {result.succeeded}/{total_tasks}")
    print(f"Failed:          {result.failed}")
    print(f"Throughput:      {result.throughput:.2f} tasks/sec")
    print("=" * 60 + "\n")


def main():
    parser = argparse.ArgumentParser(
        description="Flame cluster benchmark using core API"
    )
    parser.add_argument(
        "--sessions",
        type=int,
        default=NUM_SESSIONS,
        help=f"Number of concurrent sessions (default: {NUM_SESSIONS})",
    )
    parser.add_argument(
        "--tasks",
        type=int,
        default=TASKS_PER_SESSION,
        help=f"Tasks per session (default: {TASKS_PER_SESSION})",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=TIMEOUT_SECS,
        help=f"Timeout in seconds (default: {TIMEOUT_SECS})",
    )
    args = parser.parse_args()

    total_tasks = args.sessions * args.tasks

    registered = setup_benchmark_app()
    try:
        result = run_benchmark(
            num_sessions=args.sessions,
            tasks_per_session=args.tasks,
        )
    finally:
        teardown_benchmark_app(registered)

    print_results(result, total_tasks)

    assert result.failed == 0, f"Benchmark had {result.failed} failed tasks"
    assert result.succeeded == total_tasks, (
        f"Not all tasks succeeded: {result.succeeded}/{total_tasks}"
    )
    assert result.duration_secs < args.timeout, (
        f"Benchmark exceeded {args.timeout}s timeout: {result.duration_secs:.2f}s"
    )

    print("✓ Benchmark PASSED")


if __name__ == "__main__":
    main()
