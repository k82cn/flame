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

Runner API Example
==================

This example demonstrates how to use the Flame Runner API to simplify
Python application deployment and execution.

Prerequisites:
1. Configure flame.yaml with package storage (see docs/runner-setup.md)
2. Ensure flmrun application is registered
3. Create storage directory and ensure it's accessible

Usage:
    cd /opt/e2e
    uv run python runner_example.py
"""

from flamepy import Runner


# Example 1: Simple function
def sum_fn(a: int, b: int) -> int:
    """Sum two integers."""
    return a + b


# Example 2: Class with state
class Counter:
    """A stateful counter class."""
    
    def __init__(self, initial: int = 0):
        self._count = initial
    
    def add(self, a: int) -> int:
        """Add to the counter."""
        self._count = self._count + a
        return self._count
    
    def get_counter(self) -> int:
        """Get the current counter value."""
        return self._count


# Example 3: Calculator class
class Calculator:
    """A simple calculator."""
    
    def add(self, a: int, b: int) -> int:
        return a + b
    
    def multiply(self, a: int, b: int) -> int:
        return a * b
    
    def subtract(self, a: int, b: int) -> int:
        return a - b


def main():
    """Run all examples."""
    
    print("=" * 60)
    print("Flame Runner API Examples")
    print("=" * 60)
    
    # Example 1: Simple function
    print("\n[Example 1] Simple Function")
    print("-" * 60)
    with Runner("example-sum") as rr:
        sum_service = rr.service(sum_fn)
        result = sum_service(10, 20)
        print(f"sum_fn(10, 20) = {result.get()}")
    
    # Example 2: Class with auto-instantiation
    print("\n[Example 2] Class with Auto-Instantiation")
    print("-" * 60)
    with Runner("example-counter-class") as rr:
        cnt_s = rr.service(Counter)
        cnt_s.add(1)
        cnt_s.add(3)
        result = cnt_s.get_counter()
        print(f"Counter: 0 + 1 + 3 = {result.get()}")
    
    # Example 3: Class instance
    print("\n[Example 3] Class Instance")
    print("-" * 60)
    with Runner("example-counter-instance") as rr:
        cnt_os = rr.service(Counter(10))
        cnt_os.add(1)
        cnt_os.add(3)
        result = cnt_os.get_counter()
        print(f"Counter: 10 + 1 + 3 = {result.get()}")
    
    # Example 4: ObjectFuture as arguments
    print("\n[Example 4] ObjectFuture as Arguments")
    print("-" * 60)
    with Runner("example-objectfuture") as rr:
        cnt_os = rr.service(Counter(10))
        cnt_os.add(1)
        cnt_os.add(3)
        res_r = cnt_os.get_counter()
        
        # Pass ObjectFuture as argument (efficient for large objects)
        print(f"Counter: 10 + 1 + 3 = {res_r.get()}")
        cnt_os.add(res_r)
        res_r2 = cnt_os.get_counter()
        print(f"Counter: 14 + 14 = {res_r2.get()}")
    
    # Example 5: Multiple services
    print("\n[Example 5] Multiple Services")
    print("-" * 60)
    with Runner("example-multi") as rr:
        sum_service = rr.service(sum_fn)
        calc_service = rr.service(Calculator())
        
        result1 = sum_service(5, 3)
        result2 = calc_service.multiply(4, 7)
        
        print(f"sum_fn(5, 3) = {result1.get()}")
        print(f"Calculator.multiply(4, 7) = {result2.get()}")
    
    # Example 6: Keyword arguments
    print("\n[Example 6] Keyword Arguments")
    print("-" * 60)
    
    def greet(name: str, greeting: str = "Hello") -> str:
        """Greet someone."""
        return f"{greeting}, {name}!"
    
    with Runner("example-kwargs") as rr:
        greet_service = rr.service(greet)
        
        result1 = greet_service(name="World", greeting="Hi")
        result2 = greet_service(name="Python")
        
        print(f"greet(name='World', greeting='Hi') = {result1.get()}")
        print(f"greet(name='Python') = {result2.get()}")
    
    print("\n" + "=" * 60)
    print("All examples completed successfully!")
    print("=" * 60)


if __name__ == "__main__":
    main()
