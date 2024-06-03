import os
import time
import math
import json
import logging

import pygal
from dotenv import load_dotenv
from azure.cosmos import exceptions
from pydantic import BaseModel, create_model
from fastapi import Depends, FastAPI, Request, Response, HTTPException
from fastapi.staticfiles import StaticFiles
from fastapi.responses import HTMLResponse, RedirectResponse, JSONResponse
from starlette.middleware.sessions import SessionMiddleware
from authlib.integrations.starlette_client import OAuth
from jinjax.catalog import Catalog

from models import Users, Projects, Invocations, Endpoints, payload_model
from cosmos import CosmosConnection
from ai import ai_function, model2credits

# from llm import llm_openai
from auth import auth_router, authenticate, get_user, get_project

load_dotenv()

logging.getLogger("azure").setLevel(logging.WARNING)
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

catalog = Catalog()
catalog.add_folder("components")
catalog.add_folder("pages")

app = FastAPI()
app.state.session_store = {}
app.include_router(auth_router)
app.add_middleware(SessionMiddleware, secret_key="your-secret-key")
app.mount("/static", StaticFiles(directory="static"), name="static")

cosmos = CosmosConnection.from_connection_string(
    os.environ["COSMOS_CONNECTION_STRING"], "caipi-db"
)


def invocation_chart(invocations: list[Invocations]) -> str:
    # Process data for Pygal
    invocation_counts = {}
    for invocation in invocations:
        project = invocation.project
        date = invocation.timestamp.strftime("%Y-%m-%d")

        if project not in invocation_counts:
            invocation_counts[project] = {}
        if date not in invocation_counts[project]:
            invocation_counts[project][date] = 0

        invocation_counts[project][date] += 1

    style = pygal.style.Style(
        background="transparent",
        plot_background="transparent",
        colors=("#A9D80D", "#439C3A", "#122E38", "#DEEBE1"),
    )

    # Create Pygal Area chart
    area_chart = pygal.StackedLine(
        fill=True,
        height=250,
        width=960,
        interpolate="cubic",
        show_legend=False,
        x_label_rotation=0,
        show_minor_y_labels=False,
        style=style,
    )
    area_chart.x_labels = sorted(
        {date for project_data in invocation_counts.values() for date in project_data}
    )

    for project, counts in invocation_counts.items():
        values = [counts.get(date, 0) for date in area_chart.x_labels]
        area_chart.add(project, values)

    # Render the chart to an SVG string
    return area_chart.render(is_unicode=True)


@app.exception_handler(HTTPException)
async def unauthorized_exception_handler(request: Request, exc: HTTPException):
    if exc.status_code == 401:
        return RedirectResponse(url="/login")
    return JSONResponse(
        status_code=exc.status_code,
        content={"detail": exc.detail},
    )


@app.middleware("http")
async def log_execution_time(request: Request, call_next):
    start_time = time.time()
    response = await call_next(request)
    process_time = int((time.time() - start_time) * 1000)
    logger.info(f"{request.url.path} executed in {process_time}ms")
    return response


@app.get("/health")
async def health():
    return "I'm healthy."


@app.get("/", response_class=HTMLResponse)
async def landing_page():
    return catalog.render("Landing")


@app.get("/login", response_class=HTMLResponse)
async def login():
    return catalog.render("Login")


@app.get("/app", response_class=HTMLResponse)
async def dashboard(user: Users = Depends(get_user)):
    projects = Projects.find(f"user = '{user.id}'")
    invocations = Invocations.find(f"user = '{user.id}'")
    user.refresh(invocations)
    [p.refresh(invocations) for p in projects]
    chart = invocation_chart(invocations)
    return catalog.render("Dashboard", user=user, projects=projects, chart=chart)


@app.get("/app/modal/{type}/{abr}", response_class=HTMLResponse)
async def get_modal2(type: str, abr: str):
    if type == "add":
        return catalog.render("Param", prefix=abr)
    elif type == "remove":
        return ""


@app.post("/app/projects", response_class=HTMLResponse)
async def create_project(
    user: Users = Depends(get_user), project: Projects = Depends(get_project)
):
    project.save()
    endpoint = Endpoints.from_project(project, user)
    endpoint.save()
    projects = Projects.find(f"user = '{user.id}'")
    return catalog.render("DashboardMain", user=user, projects=projects)


@app.get("/app/projects/{id}", response_class=HTMLResponse)
async def read_project(id: str, user: Users = Depends(get_user)):
    project = Projects.get(id, user.id)
    invocations = Invocations.find(f"project = '{project.id}'", pk=user.id)
    project.refresh(invocations)
    return catalog.render("Project", project=project, invocations=invocations)


@app.patch("/app/projects/{id}", response_class=HTMLResponse)
async def update_project(id: str, project_new: Projects = Depends(get_project)):
    project_old = Projects.get(id, project_new.user)
    project_new.id = project_old.id
    project_new.endpoint = project_old.endpoint
    invocations = Invocations.find(f"project = '{project_new.id}'", pk=project_new.user)
    project_new.refresh(invocations)
    return catalog.render("ProjectMain", project=project_new, invocations=invocations)


@app.delete("/app/projects/{id}", response_class=HTMLResponse)
async def delete_project(id: str, user_id: str = Depends(authenticate)):
    project = Projects.get(id, user_id)
    project.delete()


@app.post("/app/invoke/{id}", response_class=HTMLResponse)
async def invoke_endpoint(id: str, req: Request, user_id: str = Depends(authenticate)):
    res_data = await invoke(id, req)
    return catalog.render(
        "InvokeResponse", payload=json.loads(res_data.body.decode("utf-8"))
    )


@app.post("/api/{id}", response_class=JSONResponse)
async def invoke(id: str, req: Request):
    try:
        endpoint = Endpoints.get(id)
    except:
        return Response("Endpoint doesn't exist", 404)
    # if endpoint.key:
    #     key = req.path_params.get("key") or req.headers.get("x-api-key") or req.auth
    #     if endpoint.key != req.path_params.
    user = Users.get(endpoint.user)
    if user.credits_used >= user.credits_avail:
        return Response("Out of Credits", 402)
    project = Projects.get(endpoint.project, endpoint.user)
    print(req.headers.get("Content-type"))
    if req.headers.get("Content-Type") == "application/json":
        data = await req.json()
    elif req.headers.get("Content-Type") in [
        "multipart/form-data",
        "application/x-www-form-urlencoded",
    ]:
        data = dict(await req.form())
    else:
        return Response("Invalid Content-Type", 422)
    try:
        request = project.request_model(**data)
    except Exception as e:
        return Response("Payload is invalid", 422)
    start = time.time()
    response = await ai_function(
        project.instructions, request, project.response_model, project.model
    )
    latency = round(time.time() - start, 3)
    chars = (
        len(response.model_dump_json())
        + len(request.model_dump_json())
        + len(project.instructions)
    )
    credits = math.ceil(chars / model2credits[project.model])
    inv = Invocations(
        project=project.id,
        user=project.user,
        credits=credits,
        latency=latency,
        success=True,
        model=project.model,
        request=request.model_dump() if project.collect_payload else None,
        response=response.model_dump() if project.collect_payload else None,
    )
    inv.save()
    user.credits_used += credits
    user.save()
    return JSONResponse(response.model_dump(), status_code=200)
