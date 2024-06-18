import os
import json
import logging
from contextlib import asynccontextmanager

from dotenv import load_dotenv
from fastapi import Depends, FastAPI, Request, HTTPException
from fastapi.staticfiles import StaticFiles
from fastapi.responses import HTMLResponse, RedirectResponse, JSONResponse
from starlette.middleware.sessions import SessionMiddleware
from jinjax.catalog import Catalog
from sqlmodel import create_engine, SQLModel

from api import api_router, invoke
from auth import auth_router, authenticate, get_user, get_project
from models import Users, Projects, Invocations, Endpoints
from sql import User, Project, Endpoint, Invocation
from cosmos import CosmosConnection
from viz import invocation_chart

load_dotenv()

logging.getLogger("azure").setLevel(logging.WARNING)
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

catalog = Catalog()
catalog.add_folder("components")
catalog.add_folder("pages")

engine = create_engine(os.environ["TURSO_CONNECTION"], echo=True)

@asynccontextmanager
async def lifespan(app: FastAPI):
    SQLModel.metadata.create_all(engine)
    yield

app = FastAPI(lifespan=lifespan)
app.state.session_store = {}
app.include_router(auth_router)
app.include_router(api_router)
app.add_middleware(SessionMiddleware, secret_key="your-secret-key")
app.mount("/static", StaticFiles(directory="static"), name="static")

cosmos = CosmosConnection.from_connection_string(
    os.environ["COSMOS_CONNECTION_STRING"], "caipi-db"
)

@app.exception_handler(HTTPException)
async def unauthorized_exception_handler(request: Request, exc: HTTPException):
    if exc.status_code == 401:
        return RedirectResponse(url="/login")
    return JSONResponse(
        status_code=exc.status_code,
        content={"detail": exc.detail},
    )


# @app.middleware("http")
# async def log_execution_time(request: Request, call_next):
#     start_time = time.time()
#     response = await call_next(request)
#     process_time = int((time.time() - start_time) * 1000)
#     logger.info(f"{request.url.path} executed in {process_time}ms")
#     return response

@app.get("/health")
async def health():
    return "I'm healthy."


@app.get("/", response_class=HTMLResponse)
async def landing_page():
    return catalog.render("Home")


@app.get("/login", response_class=HTMLResponse)
async def login():
    return catalog.render("Login")


@app.get("/terms", response_class=HTMLResponse)
async def terms():
    return catalog.render("Terms")

@app.get("/privacy", response_class=HTMLResponse)
async def privacy():
    return catalog.render("Privacy")


@app.get("/app", response_class=HTMLResponse)
async def dashboard(user: Users = Depends(get_user)):
    projects = Projects.find(f"user = '{user.id}'")
    invocations = Invocations.find(f"user = '{user.id}'")
    user.refresh(invocations)
    [p.refresh(invocations) for p in projects]
    chart = invocation_chart(invocations)
    return catalog.render("Dashboard", user=user, projects=projects, chart=chart)


@app.post("/app/projects", response_class=HTMLResponse)
async def create_project(
    user: Users = Depends(get_user), project: Projects = Depends(get_project)
):
    project.save()
    endpoint = Endpoints.from_project(project, user)
    endpoint.save()
    return RedirectResponse("/app", 303)


@app.get("/app/projects/{id}", response_class=HTMLResponse)
async def read_project(id: str, user: Users = Depends(get_user)):
    project = Projects.get(id, user.id)
    invocations = Invocations.find(f"project = '{project.id}'", n=10, pk=user.id)
    project.refresh(invocations)
    return catalog.render("Project", project=project, invocations=invocations)


@app.patch("/app/projects/{id}", response_class=HTMLResponse)
async def update_project(id: str, project_new: Projects = Depends(get_project)):
    project_old = Projects.get(id, project_new.user)
    project_new.id = project_old.id
    project_new.endpoint = project_old.endpoint
    project_new.save()
    return RedirectResponse(f"/app/projects/{id}", 303)


@app.delete("/app/projects/{id}", response_class=HTMLResponse)
async def delete_project(id: str, user_id: str = Depends(authenticate)):
    project = Projects.get(id, user_id)
    project.delete()
    return RedirectResponse("/app", 303)


@app.get("/app/modal/{type}/{abr}", response_class=HTMLResponse)
async def get_modal2(type: str, abr: str):
    if type == "add":
        return catalog.render("Param", prefix=abr)
    elif type == "remove":
        return ""


@app.post("/app/invoke/{id}", response_class=HTMLResponse)
async def invoke_endpoint(id: str, req: Request, user_id: str = Depends(authenticate)):
    res_data = await invoke(id, req)
    return catalog.render(
        "InvokeResponse", payload=json.loads(res_data.body.decode("utf-8"))
    )
