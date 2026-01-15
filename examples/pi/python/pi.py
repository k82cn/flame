"""Monte Carlo PI estimation module."""

import random


def estimate_batch(num_samples: int) -> int:
    """
    Estimate PI using Monte Carlo method with the given number of samples.

    Args:
        num_samples: Number of random points to sample

    Returns:
        Number of points inside the quarter circle
    """
    inside_circle = 0

    for _ in range(num_samples):
        # Generate random point in unit square [0,1] x [0,1]
        x = random.random()
        y = random.random()

        # Check if point is inside quarter circle (radius = 1)
        if x * x + y * y <= 1.0:
            inside_circle += 1

    return inside_circle
