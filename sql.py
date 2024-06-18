import secrets
from sqlmodel import Field, SQLModel, Relationship, JSON, create_engine
from datetime import datetime

def generate_key(n=12):
    abc = "abcdefghijklmnopqrstuvwxyz0123456789"
    return "".join(secrets.choice(abc) for _ in range(n))

class User(SQLModel, table=True):
    id: str | None = Field(default_factory=generate_key, primary_key=True)
    login: str
    provider: str
    roles: list[str] = Field(default=list, sa_type=JSON)
    email: str | None = None
    username: str | None = None
    n_invocations: int = 0
    credits_avail: int = 1000
    credits_used: int = 0
    latency: float = 0.0
    success: float = 1.0
    projects: list["Project"] = Relationship(back_populates="user")

class Project(SQLModel, table=True):
    id: str | None = Field(default_factory=generate_key, primary_key=True)
    user_id: str | None = Field(default=None, foreign_key="user.id")
    name: str
    instructions: str
    request: dict = Field(default=dict, sa_type=JSON)
    response: dict = Field(default=dict, sa_type=JSON)
    endpoint: str = Field(default_factory=generate_key)
    model: str = "gpt-35-turbo"
    temperature: float = 0.0
    collect_payload: bool = True
    ai_validation: bool = False
    n_invocations: int = 0
    credits: int = 0
    latency: float = 0.0
    success: float = 1.0
    active: bool = True
    user: User | None = Relationship(back_populates="project")
    invocations: list["Invocation"] = Relationship(back_populates="project")

class Endpoint(SQLModel, table=True):
    id: str | None = Field(default=None, primary_key=True)
    project_id: str | None = Field(default=None, foreign_key="project.id")
    user_id: str | None = Field(default=None, foreign_key="user.id")
    key: str | None = None
    project: Project | None = Relationship(back_populates="endpoint")
    user: User | None = Relationship(back_populates="endpoint")

class Invocation(SQLModel, table=True):
    id: str | None = Field(default=None, primary_key=True)
    project_id: str | None = Field(default=None, foreign_key="project.id")
    user_id: str | None = Field(default=None, foreign_key="user.id")
    credits: int
    latency: float
    success: bool
    model: str | None = None
    request: str | None = None
    response: str | None = None
    timestamp: datetime | None = Field(default=None)
    project: Project | None = Relationship(back_populates="invocation")
    user: User | None = Relationship(back_populates="invocation")


# engine = create_engine(os.environ["TURSO_CONNECTION"], echo=True)