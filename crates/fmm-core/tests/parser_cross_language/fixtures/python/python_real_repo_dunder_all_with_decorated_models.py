
from dataclasses import dataclass
from pydantic_settings import BaseSettings

__all__ = ["Settings", "DatabaseConfig", "create_engine"]

@dataclass
class DatabaseConfig:
    host: str = "localhost"
    port: int = 5432
    name: str = "app"

class Settings(BaseSettings):
    db: DatabaseConfig = DatabaseConfig()
    secret_key: str = "changeme"

def create_engine(config: DatabaseConfig):
    return f"postgresql://{config.host}:{config.port}/{config.name}"

@dataclass
class _MigrationState:
    version: int = 0

def _run_migrations():
    pass
