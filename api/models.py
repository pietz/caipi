from pydantic import Field, BaseModel, create_model

from cosmos import Document, generate_key

l2p = {
    "Text": str,
    "Number": float,
    "Boolean": bool,
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
    name: str
    user: str
    instruction: str
    request: dict
    response: dict
    endpoint: str = Field(default_factory=generate_key)
    invocations: int = 0
    chars: int = 0
    latency: float = 0.0
    success: float = 1.0
    active: bool = True

    @classmethod
    def from_form(cls, form: dict, user: Users):
        return cls(
            name=form["name"],
            user=user.id,
            instruction=form["instruction"],
            request={form["reqname"]: [form["reqtype"], form["reqdefault"]]},
            response={form["resname"]: [form["restype"], form["resdefault"]]},
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
    project: str
    user: str
    request: dict
    response: dict
    chars: int
    latency: float
    success: bool
