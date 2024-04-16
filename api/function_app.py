import os
import time
import logging

import azure.functions as func
from azure.cosmos import exceptions
from pydantic import BaseModel

from models import Users, Projects, Invocations, Endpoints, payload_model
from cosmos import CosmosConnection
from utils import authenticate
from views import dashboard, modal, param, project_page
from ai import ai_function

logging.getLogger("azure.cosmos").setLevel(logging.ERROR)


app = func.FunctionApp(http_auth_level=func.AuthLevel.FUNCTION)
cosmos = CosmosConnection.from_connection_string(
    os.environ["COSMOS_CONNECTION_STRING"], "caipi-db"
)


@app.route(route="health", methods=["GET", "POST"])
async def health(req: func.HttpRequest) -> func.HttpResponse:
    return func.HttpResponse("OK", status_code=200)


@app.route(route="app", methods=["GET"])
async def dash(req: func.HttpRequest) -> func.HttpResponse:
    try:
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
    except Exception as e:
        logging.error(str(e))
        return func.HttpResponse(str(e), status_code=512)


@app.route(route="app/{project_id}", methods=["GET"])
async def proj(req: func.HttpRequest) -> func.HttpResponse:
    project_id = req.route_params["project_id"]
    try:
        user = authenticate(req)
        if not user:
            return func.HttpResponse("Unauthorized", status_code=401)
        project = Projects.get(project_id, user.id)
        invocations = Invocations.find(f"project = '{project_id}'", pk=user.id)
        project.refresh(invocations)
        return func.HttpResponse(
            str(project_page(project, invocations)), status_code=200
        )
    except Exception as e:
        logging.error(str(e))
        return func.HttpResponse(str(e), status_code=512)


@app.route(route="modal/{type}/{abr}", methods=["GET"])
async def get_modal(req: func.HttpRequest) -> func.HttpResponse:
    type = req.route_params.get("type", "")
    abr = req.route_params.get("abr", "req")
    if type == "add":
        return func.HttpResponse(str(param(abr, disabled=False)), status_code=200)
    elif type == "remove":
        return func.HttpResponse("", status_code=207)


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


@app.route("projects/{id}", methods=["DELETE"])
async def delete_project(req: func.HttpRequest):
    user = authenticate(req)
    if not user:
        return func.HttpResponse("Unauthorized", status_code=401)
    if not req.form:
        return func.HttpResponse("Bad Request", status_code=400)
    project = Projects.get(req.route_params["id"], user.id)
    project.delete()


@app.route("x/{endpoint}", methods=["POST"])
async def invoke(req: func.HttpRequest):
    endpoint = Endpoints.get(req.route_params["endpoint"])
    project = Projects.get(endpoint.project, endpoint.user)
    Request = payload_model(project.request)
    try:
        data = req.get_json()
        request = Request(**data)
    except Exception as e:
        return func.HttpResponse("Request payload is invalid", status_code=422)
    Response = payload_model(project.response)
    start = time.time()
    response: BaseModel = await ai_function(project.instructions, request, Response)
    latency = round(time.time() - start, 3)
    chars = (
        len(response.model_dump_json())
        + len(request.model_dump_json())
        + len(project.instructions)
    )
    inv = Invocations(
        project=project.id,
        user=project.user,
        chars=chars,
        latency=latency,
        success=True,
        request=request.model_dump(),
        response=response.model_dump(),
    )
    inv.save()
    return func.HttpResponse(response.model_dump_json(), status_code=200)
