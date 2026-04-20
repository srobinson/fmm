
from dataclasses import dataclass, field
from pydantic import BaseModel, Field
from fastapi import FastAPI, Depends

app = FastAPI()

@dataclass
class AppConfig:
    host: str = "0.0.0.0"
    port: int = 8000
    debug: bool = False

class RequestBody(BaseModel):
    name: str
    value: float = Field(gt=0)

@dataclass(frozen=True)
class CacheKey:
    endpoint: str
    params: str

@app.get("/health")
def health_check():
    return {"status": "ok"}

@app.post("/process")
def process_item(body: RequestBody):
    return {"received": body.name}

def _internal_validator(data):
    return data is not None
