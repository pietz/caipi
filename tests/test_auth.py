import pytest
from fastapi.testclient import TestClient
from unittest.mock import patch, MagicMock
from sqlmodel import Session

from app import app
from auth import authenticate, payload_from_form, get_project
from sql import User, Project

client = TestClient(app)

@pytest.fixture
def mock_session():
    return MagicMock(spec=Session)

def test_authenticate_valid_session():
    request = MagicMock()
    request.cookies = {"session_id": "test_session"}
    request.app.state.session_store = {"test_session": "test_user_id"}
    
    result = authenticate(request)
    assert result == "test_user_id"

def test_authenticate_invalid_session():
    request = MagicMock()
    request.cookies = {"session_id": "invalid_session"}
    request.app.state.session_store = {}
    
    with pytest.raises(HTTPException) as exc_info:
        authenticate(request)
    assert exc_info.value.status_code == 401

def test_payload_from_form():
    form_data = MagicMock()
    form_data.getlist.side_effect = [
        ["name1", "name2"],
        ["string", "integer"]
    ]
    
    result = payload_from_form(form_data, "test")
    assert result == {
        "name1": ["string", None],
        "name2": ["integer", None]
    }

@patch("auth.authenticate")
async def test_get_project(mock_authenticate, mock_session):
    mock_authenticate.return_value = "test_user_id"
    
    form_data = {
        "name": "Test Project",
        "instructions": "Test Instructions",
        "req_name": ["req1", "req2"],
        "req_dtype": ["string", "integer"],
        "res_name": ["res1"],
        "res_dtype": ["boolean"],
        "collect_payload": "on",
        "ai_validation": "on",
        "model": "gpt-4",
        "temperature": "0.5"
    }
    
    request = MagicMock()
    request.form.return_value = form_data
    
    project = await get_project(request)
    
    assert project.user_id == "test_user_id"
    assert project.name == "Test Project"
    assert project.instructions == "Test Instructions"
    assert project.request == {"req1": ["string", None], "req2": ["integer", None]}
    assert project.response == {"res1": ["boolean", None]}
    assert project.collect_payload == True
    assert project.ai_validation == True
    assert project.model == "gpt-4"
    assert project.temperature == 0.5

@pytest.mark.asyncio
async def test_logout():
    response = await client.get("/logout")
    assert response.status_code == 307  # Temporary Redirect
    assert response.headers["location"] == "/"
    assert "session_id" in response.headers["set-cookie"]
    assert "Max-Age=0" in response.headers["set-cookie"]

@pytest.mark.asyncio
@patch("auth.oauth.github.authorize_redirect")
async def test_github_login(mock_authorize_redirect):
    mock_authorize_redirect.return_value = "http://mock-redirect-url"
    
    response = await client.get("/github/login")
    assert response.status_code == 200
    mock_authorize_redirect.assert_called_once()

@pytest.mark.asyncio
@patch("auth.oauth.github.authorize_access_token")
@patch("auth.oauth.github.userinfo")
async def test_github_callback(mock_userinfo, mock_authorize_access_token, mock_session):
    mock_authorize_access_token.return_value = "mock_token"
    mock_userinfo.return_value = {
        "id": "12345",
        "login": "testuser",
        "name": "Test User",
        "email": "testuser@example.com"
    }
    
    response = await client.get("/github/callback")
    assert response.status_code == 307  # Temporary Redirect
    assert response.headers["location"] == "/"
    
    mock_session.query.return_value.filter.return_value.first.return_value = None
    mock_session.add.assert_called_once()
    mock_session.commit.assert_called_once()

@pytest.mark.asyncio
@patch("auth.oauth.google.authorize_redirect")
async def test_google_login(mock_authorize_redirect):
    mock_authorize_redirect.return_value = "http://mock-redirect-url"
    
    response = await client.get("/google/login")
    assert response.status_code == 200
    mock_authorize_redirect.assert_called_once()

@pytest.mark.asyncio
@patch("auth.oauth.google.authorize_access_token")
@patch("auth.oauth.google.userinfo")
async def test_google_callback(mock_userinfo, mock_authorize_access_token, mock_session):
    mock_authorize_access_token.return_value = "mock_token"
    mock_userinfo.return_value = {
        "sub": "12345",
        "email": "testuser@example.com",
        "name": "Test User"
    }
    
    response = await client.get("/google/callback")
    assert response.status_code == 307  # Temporary Redirect
    assert response.headers["location"] == "/"
    
    mock_session.query.return_value.filter.return_value.first.return_value = None
    mock_session.add.assert_called_once()
    mock_session.commit.assert_called_once()

@pytest.mark.asyncio
async def test_login_page():
    response = await client.get("/login")
    assert response.status_code == 200
    assert "Login" in response.text

@pytest.mark.asyncio
@patch("auth.authenticate")
async def test_projects_page(mock_authenticate, mock_session):
    mock_authenticate.return_value = "test_user_id"
    mock_session.query.return_value.filter.return_value.all.return_value = [
        Project(id=1, name="Project 1", user_id="test_user_id"),
        Project(id=2, name="Project 2", user_id="test_user_id")
    ]
    
    response = await client.get("/projects")
    assert response.status_code == 200
    assert "Project 1" in response.text
    assert "Project 2" in response.text

@pytest.mark.asyncio
@patch("auth.authenticate")
@patch("auth.get_project")
async def test_create_project(mock_get_project, mock_authenticate, mock_session):
    mock_authenticate.return_value = "test_user_id"
    mock_project = MagicMock()
    mock_get_project.return_value = mock_project
    
    response = await client.post("/projects/create", data={})
    assert response.status_code == 307  # Temporary Redirect
    assert response.headers["location"] == "/projects"
    
    mock_session.add.assert_called_once_with(mock_project)
    mock_session.commit.assert_called_once()

@pytest.mark.asyncio
@patch("auth.authenticate")
async def test_delete_project(mock_authenticate, mock_session):
    mock_authenticate.return_value = "test_user_id"
    mock_project = MagicMock()
    mock_session.query.return_value.filter.return_value.first.return_value = mock_project
    
    response = await client.post("/projects/1/delete")
    assert response.status_code == 307  # Temporary Redirect
    assert response.headers["location"] == "/projects"
    mock_session.delete.assert_called_once_with(mock_project)
    mock_session.commit.assert_called_once()

@pytest.mark.asyncio
@patch("auth.authenticate")
async def test_get_modal2_add(mock_authenticate):
    mock_authenticate.return_value = "test_user_id"
    
    response = await client.get("/app/modal/add/req")
    assert response.status_code == 200
    assert "Param" in response.text
    assert "req" in response.text

@pytest.mark.asyncio
@patch("auth.authenticate")
async def test_get_modal2_remove(mock_authenticate):
    mock_authenticate.return_value = "test_user_id"
    
    response = await client.get("/app/modal/remove/res")
    assert response.status_code == 200
    assert response.text == ""

@pytest.mark.asyncio
@patch("auth.authenticate")
@patch("app.invoke")
async def test_invoke_endpoint(mock_invoke, mock_authenticate, mock_session):
    mock_authenticate.return_value = "test_user_id"
    mock_invoke.return_value = JSONResponse(content={"result": "success"})
    
    response = await client.post("/app/invoke/1", json={})
    assert response.status_code == 200
    assert "success" in response.text

@pytest.mark.asyncio
@patch("app.ai_function")
async def test_invoke(mock_ai_function, mock_session):
    mock_project = MagicMock()
    mock_project.id = "1"
    mock_project.user_id = "test_user_id"
    mock_project.instructions = "Test instructions"
    mock_project.model = "gpt-3.5-turbo"
    mock_project.collect_payload = True
    
    mock_user = MagicMock()
    mock_user.n_credits_avail = 100
    
    mock_project.user = mock_user
    mock_session.get.return_value = mock_project
    
    mock_ai_function.return_value = MagicMock(model_dump=lambda: {"result": "success"})
    
    response = await client.post("/api/1", json={"input": "test"})
    assert response.status_code == 200
    assert response.json() == {"result": "success"}
    
    mock_session.add.assert_called_once()
    mock_session.commit.assert_called_once()

@pytest.mark.asyncio
async def test_invoke_invalid_content_type():
    response = await client.post("/api/1", data="invalid data", headers={"Content-Type": "text/plain"})
    assert response.status_code == 422
    assert response.json()["detail"] == "Invalid Content-Type"

@pytest.mark.asyncio
@patch("app.ai_function")
async def test_invoke_out_of_credits(mock_ai_function, mock_session):
    mock_project = MagicMock()
    mock_project.id = "1"
    mock_project.user_id = "test_user_id"
    
    mock_user = MagicMock()
    mock_user.n_credits_avail = 0
    
    mock_project.user = mock_user
    mock_session.get.return_value = mock_project
    
    response = await client.post("/api/1", json={"input": "test"})
    assert response.status_code == 402
    assert response.json()["detail"] == "Out of Credits"

