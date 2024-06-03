import os
import json
from typing import Optional, Union, Any
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
    tool_choice: Optional[Union[str, dict[str, Any]]] = "auto"
    # frequency_penalty: Optional[float] = 0
    # logit_bias: Optional[Dict[str, int]] = None
    # logprobs: Optional[bool] = False
    # top_logprobs: Optional[int] = None
    # max_tokens: Optional[int] = None
    # n: Optional[int] = 1
    # presence_penalty: Optional[float] = 0
    # response_format: Optional[Dict[str, Any]] = None
    # seed: Optional[int] = None
    # stop: Optional[Union[str, List[str]]] = None
    # stream: Optional[bool] = False
    # stream_options: Optional[Dict[str, Any]] = None
    # top_p: Optional[float] = 1
    # user: Optional[str] = None


def openai_response_tool(PayloadResponse: type[BaseModel]):
    params = ToolFunctionParameters(
        type="object",
        properties=PayloadResponse.model_json_schema(),
        required=[k for k, v in PayloadResponse.model_fields.items()],
    )
    return Tool(function=ToolFunction(parameters=params))


async def llm_openai(
    model: str, instructions: str, request: BaseModel, Response: type[BaseModel]
) -> BaseModel:
    req = json.dumps(json.loads(request.model_dump_json()))
    prompt = f"<instructions>{instructions}</instructions>\n\n<data>{req}</data>"
    messages = [Message(role="user", content=prompt)]
    openai_request = OpenAIRequest(
        model=model,
        messages=messages,
        temperature=0,
        tools=[openai_response_tool(Response)],
    )
    ep = os.environ["AZURE_OPENAI_ENDPOINT"]
    api_v = os.environ["AZURE_OPENAI_API_VERSION"]
    url = f"{ep}openai/deployments/{model}/chat/completions?api-version={api_v}"
    print(openai_request.model_dump())
    async with AsyncClient() as client:
        res = await client.post(
            url,
            headers={
                "Content-Type": "application/json",
                "api-key": os.environ["AZURE_OPENAI_API_KEY"],
            },
            json=openai_request.model_dump(),
        )
    print(res.content)
    msg = res.json()["choices"][0]["message"]
    data = json.loads(msg["tool_calls"][0]["function"]["arguments"])
    return Response(**data)
