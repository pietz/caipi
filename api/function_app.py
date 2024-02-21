import json
import base64
import os

import azure.functions as func
from azure.cosmos import CosmosClient, exceptions
from jinja2 import Environment, FileSystemLoader

app = func.FunctionApp(http_auth_level=func.AuthLevel.FUNCTION)
env = Environment(loader=FileSystemLoader("templates"))
# cosmos = CosmosClient.from_connection_string(os.environ["COSMOS_CONNECTION_STRING"])
# db = cosmos.get_database_client("caipi-db")
# users = db.get_container_client("users")
# projects = db.get_container_client("projects")


def authenticate(req: func.HttpRequest):
    client_header = req.headers.get("X-MS-CLIENT-PRINCIPAL")
    if client_header:
        client_principal = json.loads(
            base64.b64decode(client_header).decode("utf-8")
        )
        if client_principal.get("userDetails"):
            return client_principal
    return None


@app.route(route="health", methods=["GET"])
def health(req: func.HttpRequest) -> func.HttpResponse:
    return func.HttpResponse("OK", status_code=200)


@app.route(route="app", methods=["GET"])
def dashboard(req: func.HttpRequest) -> func.HttpResponse:
    user = authenticate(req)
    if not user:
        return func.HttpResponse("Unauthorized", status_code=401)
    # try:
    #     user = users.read_item(user["userId"], user["userId"])
    # except exceptions.CosmosResourceNotFoundError:
    #     user = users.create_item(user)
    view = env.get_template("dashboard.html").render()
    return func.HttpResponse(view, status_code=200)

# @app.route(route="project/{id}", methods=["GET"])
# def read_project(req: func.HttpRequest) -> func.HttpResponse:
#     raise NotImplementedError

# @app.route(route="project", methods=["PUT"])
# def create_project(req: func.HttpRequest) -> func.HttpResponse:
#     raise NotImplementedError

# @app.route(route="project/{id}", methods=["DELETE"])
# def delete_project(req: func.HttpRequest) -> func.HttpResponse:
#     raise NotImplementedError

# @app.route(route="project/{id}", methods=["DELETE"])
# def delete_project(req: func.HttpRequest) -> func.HttpResponse:
#     raise NotImplementedError

# @app.route(route="call/{endpoint_id}/{key}", methods=["POST"])
# def invoke_endpoint(req: func.HttpRequest) -> func.HttpResponse:
#     # retrieve key from cosmos R
#     # retrieve user from cosmos R
#     # check if user still has credits
#     # retrieve project from cosmos R
#     # execute ai function
#     # update user credits W
#     # create invocation entry in cosmos W
#     # return result
#     raise NotImplementedError