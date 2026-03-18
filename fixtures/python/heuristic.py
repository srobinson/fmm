"""Python module without __all__ — exercises heuristic export discovery (Path B)."""

import os
from dataclasses import dataclass


@dataclass
class Agent:
    """A decorated class that should be exported."""
    name: str
    role: str


class Router:
    """A bare class that should be exported."""
    pass


def handle_request(req):
    """A bare function that should be exported."""
    return req


@staticmethod
def cached_lookup(key):
    """A decorated function that should be exported."""
    return key


MAX_CONNECTIONS = 100


def _internal_setup():
    """Private function, should NOT be exported."""
    pass


class _Registry:
    """Private class, should NOT be exported."""
    pass
