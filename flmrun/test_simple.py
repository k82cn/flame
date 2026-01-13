#!/usr/bin/env python3
"""
Simple test case from RFE284-flmrun.md design document.
This demonstrates the basic usage of flmrun with a sum function.
"""

import sys
sys.path.insert(0, '../sdk/python/src')

import flamepy


def sum(a: int, b: int) -> int:
    """Sum two integers."""
    return a + b


def main():
    print("Running simple flmrun test from design document...")
    
    # Step 2: Create a session with RunnerContext and sum
    ctx = flamepy.RunnerContext(execution_object=sum)
    ssn = flamepy.create_session("flmrun", ctx)
    
    print(f"Session created: {ssn.id}")
    
    try:
        # Step 3: Invoke the sum function remotely
        req = flamepy.RunnerRequest(method=None, args=(1, 2))
        task = ssn.invoke(req)
        
        print(f"Task created: {task.id}")
        
        # Step 4: Get and print the result
        result = task.get()
        print(f"Result: {result}")
        
        # Verify the result
        assert result == 3, f"Expected 3, got {result}"
        print("✓ Test PASSED - Result is 3 as expected!")
        
    finally:
        # Clean up
        ssn.close()
        print("Session closed")


if __name__ == "__main__":
    main()
