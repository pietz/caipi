import os
import json

import azure.functions as func
from azure.cosmos import exceptions
# from jinja2 import Environment, FileSystemLoader

from models import Users, Projects, Invocations, Endpoints, payload_model
from cosmos import CosmosConnection
from utils import authenticate
from views import dashboard
from ai import ai_function

app = func.FunctionApp(http_auth_level=func.AuthLevel.FUNCTION)
# env = Environment(loader=FileSystemLoader("templates"))
cosmos = CosmosConnection.from_connection_string(
    os.environ["COSMOS_CONNECTION_STRING"], "caipi-db"
)


@app.route(route="health", methods=["GET", "POST"])
async def health(req: func.HttpRequest) -> func.HttpResponse:
    return func.HttpResponse("OK", status_code=200)


# @app.route(route="app", methods=["GET"])
# def dashboardx(req: func.HttpRequest) -> func.HttpResponse:
#     user = authenticate(req)
#     if not user:
#         return func.HttpResponse("Unauthorized", status_code=401)
#     try:
#         user = Users.get(user.id)
#     except exceptions.CosmosResourceNotFoundError:
#         user.save()
#     # p = Projects(
#     #     name="Test Project",
#     #     user=user.id,
#     #     instruction="Translate the input text between English and German depending on the lamguage of the input.",
#     #     request={"input": ["str", ""]},
#     #     response={"output": ["str", ""]},
#     # )
#     # p.save()
#     # e = Endpoints.from_project(p, user)
#     # e.save()
#     # projects = [p]
#     # invocations = []
#     projects = Projects.find(f"user = '{user.id}'")
#     print(projects)
#     invocations = Invocations.find(f"user = '{user.id}'")
#     print(invocations)
#     view = render_template(
#         env,
#         "dashboard.html",
#         user=user.model_dump(),
#         projects=[p.model_dump() for p in projects],
#     )
#     return func.HttpResponse(view, status_code=200)


@app.route(route="app", methods=["GET"])
async def dashboardx(req: func.HttpRequest) -> func.HttpResponse:
    user = authenticate(req)
    if not user:
        return func.HttpResponse("Unauthorized", status_code=401)
    try:
        user = Users.get(user.id)
    except exceptions.CosmosResourceNotFoundError:
        user.save()
    projects = Projects.find(f"user = '{user.id}'")
    invocations = Invocations.find(f"user = '{user.id}'")
    user.refresh(invocations)
    [p.refresh(invocations) for p in projects]
    return func.HttpResponse(str(dashboard(user, projects)), status_code=200)


# @app.route(route="projects/{id}", methods=["GET"])
# def get_project(req: func.HttpRequest):
#     user = authenticate(req)
#     if not user:
#         return func.HttpResponse("Unauthorized", status_code=401)
#     project = Projects.get(req.route_params["id"], user.id)
#     invocations = Invocations.find(f"project = '{project.id}'")
#     return func.HttpResponse(str(dashboard(user, invocations)), status_code=200)


@app.route("projects", methods=["POST"])
async def create_project(req: func.HttpRequest):
    user = authenticate(req)
    if not user:
        return func.HttpResponse("Unauthorized", status_code=401)
    if not req.form:
        return func.HttpResponse("Bad Request", status_code=400)
    project = Projects.from_form(req.form, user)
    project.save()
    endpoint = Endpoints.from_project(project, user)
    endpoint.save()
    projects = Projects.find(f"user = '{user.id}'")
    return func.HttpResponse(str(dashboard(user, projects)), status_code=200)


@app.route("x/{endpoint}", methods=["POST"])
async def invoke(req: func.HttpRequest):
    endpoint = Endpoints.get(req.route_params["endpoint"])
    project = Projects.get(endpoint.project, endpoint.user)

    Request = payload_model(project.request)
    print(Request)
    try:
        data = req.get_json()
        inputs = Request(**data)
    except Exception as e:
        return func.HttpResponse("Request payload is invalid", status_code=422)
    Response = payload_model(project.response)
    res = await ai_function(project.instruction, inputs, Response)
    return func.HttpResponse(res.model_dump_json(), status_code=200)


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
