import os
import json
from typing import Optional, Union, Any, Tuple
from pydantic import BaseModel
from httpx import AsyncClient


class Message(BaseModel):
    role: str
    content: str


class ToolFunctionParameters(BaseModel):
    type: str = "object"
    properties: dict[str, Any]
    required: list[str]


class ToolFunction(BaseModel):
    name: str = "structured_response"
    description: str = "Saves the response in a structured format."
    parameters: ToolFunctionParameters


class Tool(BaseModel):
    type: str = "function"
    function: ToolFunction


class OpenAIRequest(BaseModel):
    model: str | None = None
    messages: list[Message]
    temperature: Optional[float] = 1
    tools: Optional[list[Tool]] = None
    tool_choice: str = "auto"


def openai_response_tool(PayloadResponse: type[BaseModel]):
    schema = PayloadResponse.model_json_schema()
    schema.pop("title")
    params = ToolFunctionParameters(
        type="object",
        properties={"response": schema},
        required=[k for k, v in PayloadResponse.model_fields.items()],
    )
    return Tool(function=ToolFunction(parameters=params))


async def llm_openai(
    model: str, instructions: str, request: BaseModel, Response: type[BaseModel]
) -> Tuple[BaseModel | None, int]:
    req = json.dumps(json.loads(request.model_dump_json()))
    prompt = f"<instructions>{instructions}</instructions>\n\n<data>{req}</data>"
    messages = [Message(role="user", content=prompt)]
    openai_request = OpenAIRequest(
        model=model,
        messages=messages,
        temperature=0,
        tools=[openai_response_tool(Response)],
    )
    print(openai_request.model_dump())
    ep = os.environ["AZURE_OPENAI_ENDPOINT"]
    api_v = os.environ["AZURE_OPENAI_API_VERSION"]
    url = f"{ep}openai/deployments/{model}/chat/completions?api-version={api_v}"
    async with AsyncClient() as client:
        res = await client.post(
            url,
            headers={
                "Content-Type": "application/json",
                "api-key": os.environ["AZURE_OPENAI_API_KEY"],
            },
            json=openai_request.model_dump(),
        )
    if res.status_code >= 300:
        return (None, res.status_code)
    msg = res.json()["choices"][0]["message"]
    print(msg)
    data = json.loads(msg["tool_calls"][0]["function"]["arguments"])
    print(data)
    return (Response(**data), res.status_code)
