import math
import time
import json
import logging
from contextlib import asynccontextmanager

from dotenv import load_dotenv
from fastapi import Depends, FastAPI, Request, HTTPException
from fastapi.staticfiles import StaticFiles
from fastapi.responses import HTMLResponse, RedirectResponse, JSONResponse
from starlette.middleware.sessions import SessionMiddleware
from jinjax.catalog import Catalog
from sqlmodel import SQLModel, Session

from auth import auth_router, authenticate, get_project
from sql import User, Project, Invocation, engine, get_db
from ai import ai_function, model2credits
from utils import req_to_data
from viz import invocation_chart

load_dotenv()

logging.getLogger("azure").setLevel(logging.WARNING)
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

catalog = Catalog()
catalog.add_folder("components")
catalog.add_folder("pages")

@asynccontextmanager
async def lifespan(app: FastAPI):
    SQLModel.metadata.create_all(engine)
    yield

app = FastAPI(lifespan=lifespan)
app.state.session_store = {}
app.include_router(auth_router)
# TODO Replace with Redis or similar
app.add_middleware(SessionMiddleware, secret_key="your-secret-key")
app.mount("/static", StaticFiles(directory="static"), name="static")

@app.exception_handler(HTTPException)
async def unauthorized_exception_handler(request: Request, exc: HTTPException):
    if exc.status_code == 401:
        return RedirectResponse(url="/login")
    return JSONResponse(
        status_code=exc.status_code,
        content={"detail": exc.detail},
    )

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
async def dashboard(user_id = Depends(authenticate), db: Session = Depends(get_db)):
    user = db.get(User, user_id)
    if user is None:
        raise HTTPException(status_code=404)
    # TODO Refresh User and Project obejcts in the DB
    chart = invocation_chart(user.invocations)
    return catalog.render("Dashboard", user=user, projects=user.projects, chart=chart)


@app.post("/app/projects", response_class=HTMLResponse)
async def create_project(project: Project = Depends(get_project), db: Session = Depends(get_db)):
    db.add(project)
    db.commit()
    return RedirectResponse("/app", 303)



@app.get("/app/projects/{id}", response_class=HTMLResponse)
async def read_project(
    id: str,
    user_id: str = Depends(authenticate),
    db: Session = Depends(get_db)
):
    project = db.get(Project, id)
    if project is None:
        raise HTTPException(status_code=404)
    invocations = list(reversed(project.invocations))
    return catalog.render("Project", project=project, invocations=invocations)

@app.patch("/app/projects/{id}", response_class=HTMLResponse)
async def update_project(
    id: str,
    project_update: Project = Depends(get_project),
    db: Session = Depends(get_db)
):
    project = db.get(Project, id)
    if not project:
        raise HTTPException(status_code=404)
    for key, value in project_update.model_dump(exclude_unset=True).items():
        setattr(project, key, value)
    db.add(project)
    db.commit()
    return RedirectResponse(f"/app/projects/{id}", 303)

@app.delete("/app/projects/{id}", response_class=HTMLResponse)
async def delete_project(
    id: str,
    user_id: str = Depends(authenticate),
    db: Session = Depends(get_db)
):
    project = db.get(Project, id)
    if project is None:
        raise HTTPException(status_code=404)
    for invocation in project.invocations:
        db.delete(invocation)
    db.delete(project)
    db.commit()
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
    # using with statement instead because ai_function runs long
    with Session(engine) as session:
        project = session.get(Project, id)
        if project is None:
            raise HTTPException(status_code=404)
        invocations = project.invocations
    invocations = list(reversed(invocations))
    return catalog.render(
        "InvokeReturn", payload=json.loads(res_data.body.decode("utf-8")), invocations=invocations
    )


@app.post("/api/{id}", response_class=JSONResponse)
async def invoke(id: str, req: Request):
    # using with statement instead because ai_function runs long
    with Session(engine) as session:
        project = session.get(Project, id)
        if project is None:
            raise HTTPException(status_code=404)
        user = project.user
        if not user:
            raise HTTPException(status_code=404)
        
    if user.n_credits_avail <= 0:
        raise HTTPException(status_code=402, detail="Out of Credits")

    data = await req_to_data(req)
    try:
        request = project.request_model(**data)
    except Exception as e:
        raise HTTPException(status_code=422, detail="Payload is invalid.")
    
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
    user.n_credits_avail = max(user.n_credits_avail - credits, 0)
    inv = Invocation(
        project_id=project.id,
        user_id=project.user_id,
        n_credits_used=credits,
        latency_sec=latency,
        status_code=200,
        model=project.model,
        request=request.model_dump() if project.collect_payload else None,
        response=response.model_dump() if project.collect_payload else None,
    )
    with Session(engine) as session:
        session.add(inv)
        session.commit()
    return JSONResponse(response.model_dump(), 200)