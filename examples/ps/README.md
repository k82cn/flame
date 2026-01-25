# Parameter Server Example

This example demonstrates distributed training using the **Parameter Server** pattern with Flame's Python SDK. It trains a simple convolutional neural network on the MNIST dataset using synchronous gradient updates.

## Overview

The parameter server pattern is a classic distributed training architecture where:
- A **Parameter Server** maintains the global model weights and applies gradient updates
- Multiple **Data Workers** compute gradients on different data batches in parallel
- Workers fetch the latest weights, compute gradients, and send them back to the parameter server

This example uses Flame's `flamepy.rl.Runner` to orchestrate the distributed services and handle inter-service communication.

## Architecture

```
┌─────────────────────┐
│  Parameter Server   │  - Stores model weights
│                     │  - Applies aggregated gradients
└──────────┬──────────┘
           │
    ┌──────┴──────┐
    │             │
┌───▼────┐   ┌───▼────┐
│Worker 1│   │Worker 2│  - Fetch weights
│        │   │        │  - Compute gradients on data batches
└────────┘   └────────┘
```

### Components

1. **ConvNet**: A small convolutional neural network for MNIST digit classification
2. **ParameterServer**: Maintains model state and applies gradient updates using SGD
3. **DataWorker**: Computes gradients on mini-batches from the training dataset
4. **Main Training Loop**: Coordinates synchronous training iterations

## Files

- `main.py`: Entry point that sets up the runner and training loop
- `ps.py`: Implementation of the model, parameter server, and data workers
- `pyproject.toml`: Project dependencies and configuration

## Requirements

- Python >= 3.12
- PyTorch and torchvision
- Flame Python SDK (`flamepy`)
- NumPy
- filelock

## How to Run

1. **Build the Flame cluster** (if not already running):
   ```bash
   docker compose build
   docker compose up -d
   ```

2. **Navigate to the example directory**:
   ```bash
   cd examples/ps
   ```

3. **Run the example**:
   ```bash
   python main.py
   ```

The script will automatically download the MNIST dataset on first run and begin training.

## Expected Output

```
100.0%
100.0%
100.0%
100.0%
Running synchronous parameter server training.
Iter 0:         accuracy is 16.5
Iter 10:        accuracy is 32.9
Final accuracy is 32.9.
```

The accuracy should improve from around 10% (random guessing) to ~85% after 20 training iterations.

## Key Concepts

### 1. Service Creation with Runner

```python
with Runner("ps-example") as rr:
    ps_svc = rr.service(ParameterServer(1e-2))
    workers_svc = [rr.service(DataWorker) for _ in range(2)]
```

The `Runner` creates and manages distributed services. Services can be instantiated from any Python class.

### 2. Asynchronous Remote Calls

```python
gradients = [worker.compute_gradients(current_weights) for worker in workers_svc]
current_weights = ps_svc.apply_gradients(*gradients).get()
```

Method calls on services return futures. Use `.get()` to block and retrieve the result.

### 3. Synchronous Training

The training loop ensures all workers compute gradients before the parameter server applies updates:

```python
for i in range(20):
    # Start all gradient computations in parallel
    gradients = [worker.compute_gradients(current_weights) for worker in workers_svc]
    # Wait for all gradients and apply update
    current_weights = ps_svc.apply_gradients(*gradients).get()
```

This is a **synchronous** parameter server where each iteration waits for all workers.

## Customization

### Adjust Number of Workers

Modify the worker count in `main.py`:

```python
workers_svc = [rr.service(DataWorker) for _ in range(4)]  # Use 4 workers
```

### Change Learning Rate

Pass a different learning rate to the ParameterServer:

```python
ps_svc = rr.service(ParameterServer(1e-3))  # Lower learning rate
```

### Increase Training Iterations

Modify the range in the training loop:

```python
for i in range(50):  # Train for 50 iterations
```

## Notes

- The example uses **filelock** to safely download MNIST data when multiple workers start simultaneously
- Evaluation is limited to 1024 samples for faster iteration during development
- The model is intentionally small to demonstrate the pattern rather than achieve state-of-the-art accuracy

## Related Examples

- For reinforcement learning examples, see the `examples/rl/` directory
- For more complex distributed patterns, check other examples in `examples/`

## Troubleshooting

If you encounter issues:

1. **Services not starting**: Ensure the Flame cluster is running with `docker compose ps`
2. **Import errors**: Rebuild containers after modifying `sdk/python`: `docker compose build`
3. **Test timeouts**: Check logs with `docker logs flame-executor-manager` and `docker logs flame-session-manager`

For more information, see the [Flame documentation](../../docs/).
