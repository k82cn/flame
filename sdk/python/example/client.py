#!/usr/bin/env python3
"""
Example usage of the Flame Python SDK.
"""

import flamepy


class MyTaskInformer(flamepy.TaskInformer):
    """Example task informer that prints task updates."""

    def on_update(self, task):
        print(f"Task {task.id}: {task.state.name}")

    def on_error(self, error):
        print(f"Error: {error}")


def main():
    print("Creating session...")
    session = flamepy.create_session(application="flmtest", common_data=b"shared data")
    print(f"Created session: {session.id}")

    # Invoke task
    print("Running task...")
    session.invoke(b"task input data", MyTaskInformer())

    # Close session
    print("Closing session...")
    session.close()

    print("Example completed successfully!")


if __name__ == "__main__":
    main()
