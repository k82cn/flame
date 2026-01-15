"""Monte Carlo PI estimation module."""

import numpy as np


def estimate_batch(num_samples: int) -> int:
    """
    Estimate PI using Monte Carlo method with the given number of samples.

    Args:
        num_samples: Number of random points to sample

    Returns:
        Number of points inside the quarter circle
    """
    x = np.random.rand(num_samples)
    y = np.random.rand(num_samples)
    inside_circle = np.sum(x*x + y*y <= 1.0)
    return int(inside_circle)
