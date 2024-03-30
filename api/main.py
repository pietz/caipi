import secrets
import time
import os

import marvin
from pydantic import BaseModel, create_model
from fastapi import FastAPI, Request, Depends, Form, HTTPException, status
from fastapi.responses import HTMLResponse, RedirectResponse, Response
from fastapi.templating import Jinja2Templates
from dotenv import load_dotenv

load_dotenv()

from .models import Users

app = FastAPI()
templates = Jinja2Templates(directory="templates")


@app.exception_handler(HTTPException)
async def http_exception_handler(request: Request, exc: HTTPException):
    if exc.status_code == status.HTTP_401_UNAUTHORIZED:
        return RedirectResponse(url="/login")
    return await request.app.default_exception_handler(request, exc)


async def catch_exceptions_middleware(request: Request, call_next):
    try:
        return await call_next(request)
    except Exception as e:
        print(e)
        return Response(str(e), status_code=512)


app.middleware("http")(catch_exceptions_middleware)


def stats(li: list):
    ep = {}
    ep["invocations"] = len(li)
    if ep["invocations"] > 0:
        ep["chars"] = sum([inv["chars"] for inv in li])
        ep["latency"] = int(sum([inv["latency"] for inv in li]) / ep["invocations"])
        ep["success"] = int(
            len([inv for inv in li if inv["success"]]) / ep["invocations"] * 100
        )
    else:
        ep["chars"], ep["latency"], ep["success"] = 0, 0, 100
    return ep


def generate_key(n=12):
    abc = "abcdefghijklmnopqrstuvwxyz0123456789"
    return "".join(secrets.choice(abc) for _ in range(n))


@app.get("/", response_class=HTMLResponse)
def landing(request: Request):
    return templates.TemplateResponse("landing.html", {"request": request})


@app.get("/app", response_class=HTMLResponse)
def dashboard(request: Request, user: dict = Depends(get_user)):
    try:
        user = users.read_item(user["id"], user["id"])
    except:
        user = users.create_item(user)
    user_id = user["id"]
    eps = projects.query_items(f"SELECT * FROM c WHERE c.user = '{user_id}'")
    eps = [x for x in eps]
    invs = invocations.query_items(f"SELECT * FROM c WHERE c.user = '{user_id}'")
    invs = [x for x in invs]
    user.update(stats(invs))
    users.upsert_item(user)
    for ep in eps:
        ep.update(stats([x for x in invs if x["endpoint"] == ep["id"]]))
    return templates.TemplateResponse(
        "dashboard.html", {"request": request, "user": user, "eps": eps}
    )


@app.get("/app/{endpoint}", response_class=HTMLResponse)
def dashboard(request: Request, endpoint: str, user: dict = Depends(get_user)):
    user = users.read_item(user["id"], user["id"])
    user_id = user["id"]
    ep = projects.read_item(endpoint, user["id"])
    invs = invocations.query_items(
        f"SELECT * FROM c WHERE c.user = '{user_id}' AND c.endpoint = '{endpoint}'"
    )
    invs = [x for x in invs]
    user.update(stats(invs))
    users.upsert_item(user)
    ep.update(stats([x for x in invs if x["endpoint"] == ep["id"]]))
    return templates.TemplateResponse(
        "project.html", {"request": request, "user": user, "ep": ep}
    )


@app.get("/login", response_class=HTMLResponse)
def login(request: Request):
    return templates.TemplateResponse("login.html", {"request": request})


@app.post("/endpoints", response_class=HTMLResponse)
def ep(
    request: Request,
    user: dict = Depends(get_user),
    name: str = Form(...),
    instruction: str = Form(...),
    inpname: list[str] = Form(...),
    inpdefault: list[str] = Form(...),
    outname: list[str] = Form(...),
    outdefault: list[str] = Form(...),
):
    user_id = user["id"]
    user = users.read_item(user_id, user_id)
    project_id = generate_key()
    endpoint_id = generate_key()
    key = generate_key()
    projects.upsert_item(
        {
            "id": project_id,
            "name": name,
            "user": user_id,
            "instruction": instruction,
            "request": {
                inpname[i]: ("str", inpdefault[i]) for i in range(len(inpname))
            },
            "response": {
                outname[i]: ("str", outdefault[i]) for i in range(len(outname))
            },
            "key": key,  # Maybe not needed
        }
    )

    endpoints.upsert_item(
        {
            "id": endpoint_id,
            "project": project_id,
            "user": user_id,
            "key": key,
        }
    )

    user_projects = projects.query_items(f"SELECT * FROM c WHERE c.user = '{user_id}'")
    user_projects = [x for x in user_projects]
    invs = invocations.query_items(f"SELECT * FROM c WHERE c.user = '{user_id}'")
    invs = [x for x in invs]
    user.update(stats(invs))
    for p in user_projects:
        p.update(stats([x for x in invs if x["endpoint"] == p["id"]]))
    return templates.TemplateResponse(
        "table.html", {"request": request, "eps": user_projects}
    )


@app.post("/api/{endpoint}/{key}")
async def x(endpoint: str, key: str, request: Request):
    ep = endpoints.read_item(endpoint, endpoint)
    if ep["key"] != key:
        raise HTTPException(status_code=403, detail="Invalid key")
    project = projects.read_item(endpoint, ep["user"])
    Req = payload_model(project["request"])
    try:
        data = await request.json()
        inputs = Req(**data)
    except Exception as e:
        raise HTTPException(status_code=422, detail="Request payload is invalid")
    Res = payload_model(project["response"])
    start = time.time()
    success = True
    res = await ai_function(project["instruction"], inputs, Res)
    duration = int((time.time() - start) * 1000)
    chars = len(inputs.model_dump_json()) + (
        len(res.model_dump_json()) if res else 1000
    )
    invocations.upsert_item(
        {
            "id": generate_key(),
            "project": project["id"],
            "user": project["user"],
            "chars": chars,
            "latency": duration,
            "success": success,
        }
    )
    return res
