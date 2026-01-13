"""
Helper functions and classes for flmrun e2e tests.
These are defined in a separate module so they can be properly pickled.
"""


def sum_func(a: int, b: int) -> int:
    """Sum two integers."""
    return a + b


def multiply_func(a: int, b: int) -> int:
    """Multiply two integers."""
    return a * b


def greet_func(name: str, greeting: str = "Hello") -> str:
    """Greet someone."""
    return f"{greeting}, {name}!"


def get_message_func() -> str:
    """Get a message."""
    return "Hello from flmrun!"


def return_dict_func(key: str, value: int) -> dict:
    """Return a dictionary."""
    return {key: value}


def return_list_func(n: int) -> list:
    """Return a list."""
    return list(range(n))


def return_tuple_func(a: int, b: str) -> tuple:
    """Return a tuple."""
    return (a, b)


def square_func(x: int) -> int:
    """Square a number."""
    return x * x


class Calculator:
    """Simple calculator class."""
    def add(self, a: int, b: int) -> int:
        return a + b
    
    def multiply(self, a: int, b: int) -> int:
        return a * b
    
    def subtract(self, a: int, b: int) -> int:
        return a - b


class Counter:
    """Stateful counter class."""
    def __init__(self):
        self.count = 0
    
    def increment(self) -> int:
        self.count += 1
        return self.count
    
    def get_count(self) -> int:
        return self.count
    
    def add(self, value: int) -> int:
        self.count += value
        return self.count
