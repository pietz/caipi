import secrets
import json
from datetime import datetime
from typing import Type, TypeVar, Any, ClassVar
from azure.cosmos import CosmosClient, ContainerProxy
from pydantic import BaseModel, Field

T = TypeVar("T", bound="Document")


def generate_key(n=12):
    abc = "abcdefghijklmnopqrstuvwxyz0123456789"
    return "".join(secrets.choice(abc) for _ in range(n))


class CosmosConnection:
    _instance = None

    def __init__(self, account_url: str, account_key: str, database_name: str):
        if CosmosConnection._instance is not None:
            raise Exception("This class is a singleton!")
        self.client = CosmosClient(account_url, account_key)
        self.database = self.client.get_database_client(database_name)
        CosmosConnection._instance = self

    @staticmethod
    def instance():
        if CosmosConnection._instance is None:
            raise Exception("Cosmos Connection is not initialized.")
        return CosmosConnection._instance

    @classmethod
    def from_connection_string(cls, connection_string: str, database_name: str):
        connection_string = connection_string.rstrip(";")
        connection_params = dict(s.split("=", 1) for s in connection_string.split(";"))
        account_url = connection_params["AccountEndpoint"]
        account_key = connection_params["AccountKey"]
        return cls(account_url, account_key, database_name)


class Document(BaseModel):
    id: str = Field(default_factory=generate_key)
    pk_field: ClassVar[str] = "id"
    container: ClassVar[ContainerProxy | None] = None

    @classmethod
    def get_container(cls) -> ContainerProxy:
        if cls.container is None:
            db = CosmosConnection.instance().database
            cls.container = db.get_container_client(cls.__name__)
        return cls.container

    def save(self) -> "Document":
        container = self.get_container()
        item = container.upsert_item(json.loads(self.model_dump_json()))
        return self.__class__(**item)

    @classmethod
    def get(cls: Type[T], id: str, pk: str | None = None) -> T:
        if pk is None and cls.pk_field != "id":
            raise Exception("Partition key `pk` is required.")
        container = cls.get_container()
        item = container.read_item(id, id if pk is None else pk)
        return cls(**item)

    @classmethod
    def find(cls: Type[T], query: str, n=1000, pk: str | None = None) -> list[T]:
        container = cls.get_container()
        full_query = f"SELECT * FROM c WHERE c.{query}"
        results = container.query_items(full_query, max_item_count=n, partition_key=pk)
        return [cls(**item) for item in results]

    def delete(self):
        container = self.get_container()
        partition_key = self._partition_key()
        container.delete_item(self.id, partition_key)

    def _partition_key(self) -> Any:
        return getattr(self, self.pk_field)
