"""Python module with __all__ — exercises Path A (explicit export list)."""

from dataclasses import dataclass

__all__ = ["Config", "DecoratedModel", "process", "APP_NAME"]

APP_NAME = "myapp"


class Config:
    """Bare class listed in __all__."""
    debug: bool = False


@dataclass
class DecoratedModel:
    """Decorated class listed in __all__ — should resolve to definition site."""
    id: int
    name: str


def process(data):
    """Bare function listed in __all__."""
    return data


@dataclass
class _InternalModel:
    """Decorated but not in __all__ — should NOT be exported."""
    temp: str


def unlisted_helper():
    """Not in __all__ — should NOT be exported."""
    pass
