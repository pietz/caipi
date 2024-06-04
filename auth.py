import os
import uuid

from dotenv import load_dotenv
from fastapi import APIRouter, Request, HTTPException, Depends
from fastapi.datastructures import FormData
from starlette.responses import RedirectResponse
from authlib.integrations.starlette_client import OAuth

from models import Users, Projects

load_dotenv()

auth_router = APIRouter()

oauth = OAuth()
oauth.register(
    name="github",
    client_id=os.environ["GITHUB_CLIENT_ID"],
    client_secret=os.environ["GITHUB_CLIENT_SECRET"],
    authorize_url="https://github.com/login/oauth/authorize",
    authorize_params=None,
    access_token_url="https://github.com/login/oauth/access_token",
    access_token_params=None,
    refresh_token_url=None,
    userinfo_endpoint="https://api.github.com/user",
    client_kwargs={"scope": "user:email"},
)


def authenticate(request: Request):
    session_id = request.cookies.get("session_id")
    if not session_id or session_id not in request.app.state.session_store:
        raise HTTPException(status_code=401)
    user_id = request.app.state.session_store[session_id]
    return user_id


def get_user(user_id: str = Depends(authenticate)):
    user = Users.get(user_id, user_id)
    if user.username != "pietz":
        raise HTTPException(status_code=401)
    return user


def payload_from_form(form: FormData, prefix: str):
    names = form.getlist(f"{prefix}_name")
    dtypes = form.getlist(f"{prefix}_dtype")
    return {name: [dtype, None] for name, dtype in zip(names, dtypes)}


async def get_project(request: Request, user_id: str = Depends(authenticate)):
    form: FormData = await request.form()
    return Projects(
        user=user_id,
        name=form.get("name"),
        instructions=form.get("instructions"),
        request=payload_from_form(form, "req"),
        response=payload_from_form(form, "res"),
        collect_payload=form.get("collect_payload", False),
        ai_validation=form.get("ai_validation", False),
        model=form.get("model", "gpt-35-turbo"),
        temperature=form.get("temperature", 0.0),
    )


@auth_router.get("/logout")
async def logout(request: Request):
    session_id = request.cookies.get("session_id")
    if session_id and session_id in request.app.state.session_store:
        del request.app.state.session_store[session_id]
    response = RedirectResponse(url="/")
    response.delete_cookie(key="session_id")
    return response


@auth_router.get("/github/login")
async def github_login(request: Request):
    redirect_url = os.environ["CAIPI_URL"] + "/github/callback"
    return await oauth.github.authorize_redirect(request, redirect_url)


@auth_router.get("/github/callback")
async def github_callback(request: Request):
    token = await oauth.github.authorize_access_token(request)
    user_info = await oauth.github.userinfo(token=token)
    session_id = str(uuid.uuid4())
    request.app.state.session_store[session_id] = str(user_info["id"])
    response = RedirectResponse(url="/app")
    response.set_cookie(key="session_id", value=session_id, httponly=True, secure=True)
    return response
