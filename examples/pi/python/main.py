"""
Monte Carlo Estimation of PI using Flame Runner API

This example uses the Monte Carlo method to estimate the value of PI by
randomly sampling points in a unit square and checking if they fall inside
a quarter circle.
"""

from flamepy import Runner
from pi import estimate_batch
import math

def main():
    """Run Monte Carlo PI estimation using distributed computing."""

    print("=" * 60)
    print("Monte Carlo Estimation of PI using Flame Runner")
    print("=" * 60)

    # Configuration
    num_batches = 10
    samples_per_batch = 1_000_000
    total_samples = num_batches * samples_per_batch

    print(f"\nConfiguration:")
    print(f"  Batches: {num_batches}")
    print(f"  Samples per batch: {samples_per_batch:,}")
    print(f"  Total samples: {total_samples:,}")
    print(f"\nRunning distributed Monte Carlo simulation...")

    # Create Runner and distribute the work
    with Runner("pi-estimation") as rr:
        # Create multiple estimator services (for parallel execution)
        estimator = rr.service(estimate_batch)

        # Submit all batch computations
        results = [estimator(samples_per_batch) for _ in range(num_batches)]

        # Collect results
        insides = [result.get() for result in results]

    # Calculate final PI estimate
    pi_estimate = 4.0 * sum(insides) / total_samples

    error = abs(pi_estimate - math.pi)
    error_percent = (error / math.pi) * 100

    print(f"\nResults:")
    print(f"  Estimated PI: {pi_estimate:.10f}")
    print(f"  Actual PI:    {math.pi:.10f}")
    print(f"  Error:        {error:.10f} ({error_percent:.6f}%)")
    print("=" * 60)


if __name__ == "__main__":
    main()
