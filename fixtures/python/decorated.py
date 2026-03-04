"""Focused decorator variants for testing decorated_definition matching."""

from dataclasses import dataclass, field
from functools import lru_cache


@dataclass
class SimpleDecorated:
    """Single decorator, no arguments."""
    name: str


@dataclass(frozen=True, slots=True)
class DecoratedWithArgs:
    """Decorator with keyword arguments."""
    value: int
    label: str = "default"


@lru_cache
@staticmethod
def multi_decorated():
    """Stacked decorators on a function."""
    return 42


def bare_function():
    """No decorator at all."""
    pass


class BareClass:
    """No decorator at all."""
    pass


@dataclass
class _PrivateDecorated:
    """Decorated but private — should NOT be exported."""
    secret: str
