import time
import math

from fastapi import APIRouter, Request, Response
from fastapi.responses import JSONResponse

from ai import ai_function, model2credits
from models import Endpoints, Users, Projects, Invocations

api_router = APIRouter()

@api_router.post("/api/{id}", response_class=JSONResponse)
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
    return JSONResponse(response.model_dump(), 200)