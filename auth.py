import os
import uuid

from dotenv import load_dotenv
from fastapi import APIRouter, Request, HTTPException, Depends
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


async def get_project(request: Request, user_id: str = Depends(authenticate)):
    form = await request.form()
    form_di = {k: form.getlist(k) for k in form.keys()}
    session_id = request.cookies.get("session_id")
    return Projects(
        user=user_id,
        name=form.get("name"),
        instructions=form.get("instructions"),
        request={
            form_di["req_name"][i]: [form_di["req_dtype"][i], None]
            for i in range(len(form_di["req_name"]))
            if form_di["req_name"][i] != ""
        },
        response={
            form_di["res_name"][i]: [form_di["res_dtype"][i], None]
            for i in range(len(form_di["res_name"]))
            if form_di["res_name"][i] != ""
        },
        active=True,  # Assuming new projects are always active by default
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
