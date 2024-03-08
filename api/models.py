# from cosmos import Document

class Users(Document):
    login: str
    provider: str
    roles: list[str]
    email: str | None = None
    username: str | None = None
    chars: int = 0
    latency: int = 0
    invocations: int = 0
    success: float = 0.0

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