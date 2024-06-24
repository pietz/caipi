import os
import secrets
from datetime import datetime
from contextlib import contextmanager
from dotenv import load_dotenv
from pydantic import BaseModel, create_model, ConfigDict
from sqlmodel import Field, SQLModel, JSON, Relationship, create_engine, Session, select

load_dotenv()

l2p = {
    "Text": str,
    "Number": float,
    "Boolean": bool,
    "str": str,
    "int": int,
    "float": float,
    "bool": bool,
    "list": list,
}

def generate_key(n=12):
    abc = "abcdefghijklmnopqrstuvwxyz0123456789"
    return "".join(secrets.choice(abc) for _ in range(n))

def payload_model(name, data) -> type[BaseModel]:
    # Maps parameter names to a tuple of (dtype, default value)
    attributes = {k: (l2p[v[0]], v[1]) for k, v in data.items()}
    config = ConfigDict(extra="forbid")
    return create_model(name, **attributes, __config__=config)

class User(SQLModel, table=True):
    id: str = Field(default=..., primary_key=True)
    login: str
    provider: str
    name: str | None = None
    email: str | None = None
    n_credits_avail: int = 1000
    n_invocations: int = 0
    latency_sec: float = 0.0
    success_rate: float = 1.0
    created: datetime = Field(default_factory=datetime.utcnow)
    projects: list["Project"] = Relationship(back_populates="user")
    invocations: list["Invocation"] = Relationship(back_populates="user")

def refresh(self, invocations: list["Invocation"]):
    self.n_credits_used = sum(inv.n_credits_used for inv in invocations)
    self.n_invocations = len(invocations)

    if self.n_invocations > 0:
        self.latency_sec = sum(inv.latency_sec for inv in invocations) / self.n_invocations
        self.success_rate = sum(1 for inv in invocations if inv.status_code < 300) / self.n_invocations


class Project(SQLModel, table=True):
    id: str | None = Field(default_factory=generate_key, primary_key=True)
    user_id: str = Field(default=None, foreign_key="user.id")
    key: str = Field(default_factory=generate_key)
    name: str
    instructions: str
    request: dict = Field(default=dict, sa_type=JSON)
    response: dict = Field(default=dict, sa_type=JSON)
    model: str = "gpt-35-turbo"
    temperature: float = 0.0
    collect_payload: bool = True
    ai_validation: bool = False
    n_invocations: int = 0
    n_credits_used: int = 0
    latency_sec: float = 0.0
    success_rate: float = 1.0
    active: bool = True
    created: datetime = Field(default_factory=datetime.utcnow)
    user: User | None = Relationship(back_populates="projects")
    invocations: list["Invocation"] = Relationship(back_populates="project")

    @property
    def request_model(self) -> type[BaseModel]:
        return payload_model("PayloadRequest", self.request)

    @property
    def response_model(self) -> type[BaseModel]:
        return payload_model("PayloadResponse", self.response)
    
    def refresh(self, invocations: list["Invocation"]):
        self.n_credits_used = sum([x.n_credits_used for x in invocations])
        self.n_invocations = len(invocations)

        if self.n_invocations > 0:
            self.latency_sec = sum([x.latency_sec for x in invocations]) / self.n_invocations
            self.success_rate = len([x for x in invocations if x.status_code < 300]) / self.n_invocations

class Invocation(SQLModel, table=True):
    id: str | None = Field(default_factory=generate_key, primary_key=True)
    user_id: str = Field(default=..., foreign_key="user.id")
    project_id: str = Field(default=..., foreign_key="project.id")
    n_credits_used: int
    latency_sec: float
    status_code: int
    model: str
    request: dict | None = Field(default=dict, sa_type=JSON)
    response: dict | None = Field(default=dict, sa_type=JSON)
    created: datetime | None = Field(default_factory=datetime.utcnow)
    user: User | None = Relationship(back_populates="invocations")
    project: Project | None = Relationship(back_populates="invocations")


engine = create_engine(os.environ["DB_CONNECTION"], echo=False)

@contextmanager
def get_session():
    session = Session(engine)
    try:
        yield session
    finally:
        session.close()

def get_db():
    with Session(engine) as session:
        yield session