"""
Copyright 2026 The Flame Authors.
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

from typing import Optional, Any
from flamepy.core.client import (
    Session,
    create_session,
    open_session,
)


class Agent:
    """A streamlined interface for building and interacting with agents.
    
    The Agent class is a thin wrapper around core.Session, where the Agent's name
    corresponds to the application's name.
    
    Example:
        Direct instantiation:
            >>> agent = Agent("test", ctx)
            >>> resp = agent.invoke(req)
            >>> agent.close()
        
        Context manager:
            >>> with Agent("test", ctx) as agent:
            ...     resp = agent.invoke(req)
    """
    
    def __init__(
        self,
        name: Optional[str] = None,
        ctx: Optional[Any] = None,
        session_id: Optional[str] = None,
        slots: int = 1,
    ):
        """Initialize an Agent.
        
        Args:
            name: Optional application name (corresponds to the agent's name).
                  Required if session_id is None.
            ctx: Optional common data (context) for the session. Only used when creating a new session.
            session_id: Optional session ID. If provided, opens an existing session.
                        Required if name is None.
            slots: Number of slots for the session (default: 1). Only used when creating a new session.
        
        Raises:
            ValueError: If both name and session_id are None.
        """
        if name is None and session_id is None:
            raise ValueError("Either 'name' or 'session_id' must be provided")
        
        self._name = name
        self._session: Optional[Session] = None
        
        if session_id is not None:
            # Open an existing session
            self._session = open_session(session_id)
            # Update name from the opened session if not provided
            if self._name is None:
                self._name = self._session.application
        else:
            # Create a new session using flamepy.create_session
            self._session = create_session(
                application=name,
                common_data=ctx,
                session_id=None,  # Let create_session generate the ID
                slots=slots,
            )
    
    def invoke(self, req: Any) -> Any:
        """Invoke the agent with a request.
        
        The request and response objects should exactly match the agent's entrypoint
        signature defined on the service side.
        
        Args:
            req: The request object matching the agent's entrypoint signature
            
        Returns:
            The response object matching the agent's entrypoint signature
            
        Example:
            >>> resp = agent.invoke(req)
        """
        if self._session is None:
            raise RuntimeError("Agent session is not initialized")
        return self._session.invoke(req)
    
    def context(self) -> Any:
        """Get the current agent context.
        
        Returns the session's shared (common) data, which represents the current
        agent context such as a running conversation or environment state.
        
        Returns:
            The common data (context) of the session, or None if not set
            
        Example:
            >>> ctx = agent.context()
        """
        if self._session is None:
            return None
        return self._session.common_data()
    
    def id(self) -> Optional[str]:
        """Get the agent's session ID.
        
        Returns:
            The session ID of the agent, or None if the session is not initialized
            
        Example:
            >>> agent_id = agent.id()
        """
        if self._session is None:
            return None
        return self._session.id
    
    def close(self) -> None:
        """Close the agent and release resources.
        
        This closes the underlying session. After calling close(), the agent
        should not be used anymore.
        """
        if self._session is not None:
            self._session.close()
            self._session = None
    
    def __enter__(self) -> "Agent":
        """Enter the context manager."""
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        """Exit the context manager and close the agent."""
        self.close()
        return False