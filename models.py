from typing import ClassVar
from datetime import datetime
from pydantic import (
    Field,
    BaseModel,
    create_model,
    ConfigDict,
    root_validator,
    computed_field,
)
from cosmos import Document, generate_key

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


def payload_model(name, data) -> type[BaseModel]:
    # Maps parameter names to a tuple of (dtype, default value)
    attributes = {k: (l2p[v[0]], v[1]) for k, v in data.items()}
    config = ConfigDict(extra="forbid")
    return create_model(name, **attributes, __config__=config)


class Users(Document):
    login: str
    provider: str
    roles: list[str]
    email: str | None = None
    username: str | None = None
    invocations: int = 0
    credits_avail: int = 1000
    credits_used: int = 0
    latency: float = 0.0
    success: float = 1.0

    @classmethod
    def from_client_principal(cls, client_principal):
        if client_principal["identityProvider"] == "github":
            user = cls(
                id=client_principal["userId"],
                login=client_principal["userDetails"],
                provider=client_principal["identityProvider"],
                roles=client_principal["userRoles"],
                username=client_principal["userDetails"],
            )
        else:
            raise NotImplementedError
        return user

    def refresh(self, invocations: list["Invocations"]):
        if len(invocations) == self.invocations:
            return  # No new entries since last time

        self.credits_used = sum([x.credits for x in invocations])
        self.invocations = len(invocations)

        if self.invocations > 0:
            self.latency = sum([x.latency for x in invocations]) / self.invocations
            self.success = len([x for x in invocations if x.success]) / self.invocations

        self.save()


class Projects(Document):
    user: str | None = None
    name: str
    instructions: str
    request: dict[str, list]
    response: dict[str, list]
    endpoint: str = Field(default_factory=generate_key)
    model: str = "gpt-35-turbo"
    temperature: float = 0.0
    collect_payload: bool = False
    ai_validation: bool = False
    invocations: int = 0
    credits: int = 0
    latency: float = 0.0
    success: float = 1.0
    active: bool = True

    pk_field: ClassVar[str] = "user"

    @property
    def request_model(self) -> type[BaseModel]:
        return payload_model("PayloadRequest", self.request)

    @property
    def response_model(self) -> type[BaseModel]:
        return payload_model("PayloadResponse", self.response)

    @classmethod
    def from_form(cls, form, user: Users):
        assert "name" in form
        assert "instructions" in form
        form_di = {k: form.getlist(k) for k in form}

        return cls(
            user=user.id,
            name=form["name"],
            instructions=form["instructions"],
            request={
                form_di["req_name"][i]: [form_di["req_dtype"][i], None]
                for i in range(len(form_di["req_name"]))
                if form_di["req_name"][i] != ""
            },
            response={
                form_di["res_name"][i]: [form_di["res_dtype"][i], None]
                for i in range(len(form_di["res_name"]))
                if form_di["res_name"][i] != ""
            },
        )

    def refresh(self, invocations: list["Invocations"]):
        invs = [inv for inv in invocations if inv.project == self.id]
        if len(invs) == self.invocations:
            return  # No new entries since last time

        self.invocations = len(invs)
        self.credits = sum([x.credits for x in invs])

        if self.invocations > 0:
            self.latency = sum([x.latency for x in invs]) / self.invocations
            self.success = len([x for x in invs if x.success]) / self.invocations

        self.save()


class Endpoints(Document):
    project: str
    user: str
    key: str | None = None

    @classmethod
    def from_project(cls, project: Projects, user: Users):
        return cls(id=project.endpoint, project=project.id, user=user.id)


class Invocations(Document):
    project: str  # TODO: Use longer ID for invocations
    user: str
    credits: int
    latency: float
    success: bool
    model: str | None = None
    request: dict | None = None
    response: dict | None = None
    timestamp: datetime | None = Field(default=None, alias="_ts")
