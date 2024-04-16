from typing import ClassVar
from pydantic import Field, BaseModel, create_model
from cosmos import Document, generate_key
from azure.functions._thirdparty.werkzeug.datastructures import ImmutableMultiDict

l2p = {
    "Text": str,
    "Number": float,
    "Boolean": bool,
    "str": str,
    "int": int,
    "float": float,
    "bool": bool,
}


def payload_model(data) -> type[BaseModel]:
    attributes = {k: (l2p[v[0]], v[1]) for k, v in data.items()}
    return create_model("Payload", **attributes)


class Users(Document):
    login: str
    provider: str
    roles: list[str]
    email: str | None = None
    username: str | None = None
    invocations: int = 0
    chars: int = 0
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

        self.chars = sum([x.chars for x in invocations])
        self.invocations = len(invocations)
        self.latency = sum([x.latency for x in invocations]) / self.invocations
        self.success = len([x for x in invocations if x.success]) / self.invocations

        self.save()


class Projects(Document):
    user: str
    name: str
    instructions: str
    request: dict[str, list]
    response: dict[str, list]
    endpoint: str = Field(default_factory=generate_key)
    model: str = "gpt-35-1106"
    invocations: int = 0
    chars: int = 0
    latency: float = 0.0
    success: float = 1.0
    active: bool = True

    pk_field: ClassVar[str] = "user"

    @classmethod
    def from_form(cls, form: ImmutableMultiDict, user: Users):
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
            },
            response={
                form_di["res_name"][i]: [form_di["res_dtype"][i], None]
                for i in range(len(form_di["res_name"]))
            },
        )

    def refresh(self, invocations: list["Invocations"]):
        invs = [inv for inv in invocations if inv.project == self.id]
        if len(invs) == self.invocations:
            return  # No new entries since last time

        self.invocations = len(invs)
        self.chars = sum([x.chars for x in invs])
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
    chars: int
    latency: float
    success: bool
    request: dict | None = None
    response: dict | None = None
