import os
import json
from typing import Any, Type
from urllib import response
import httpx
from dotenv import load_dotenv
from pydantic import BaseModel
import asyncio

load_dotenv()


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
    temperature: float | None = 0
    tools: list[Tool] | None = None
    tool_choice: str = "required"


def create_function_definition(response_model: Type[BaseModel]):
    properties = {}
    required = []

    for name, field in response_model.model_fields.items():

    for name, field in response_model.model_fields.items():
        prop = {
            "type": "string" if field.annotation == str else "object",
            "description": field.description or "",
        }
        if field.annotation != str:
            prop["properties"] = create_function_definition(field.annotation).properties
            prop["required"] = create_function_definition(field.annotation).required
        properties[name] = prop
        if field.required:
            required.append(name)

    return ToolFunctionParameters(
        type="object", properties=properties, required=required
    )


async def ai_function(
    instructions: str, request: BaseModel, response_model: Type[BaseModel]
) -> BaseModel:
    # Construct the system prompt and user message
    system_prompt = instructions
    user_message = request.model_dump_json()

    function_parameters = create_function_definition(response_model)
    tool_function = ToolFunction(
        name="structured_response",
        description="Saves the response in a structured format.",
        parameters=function_parameters,
    )
    tool = Tool(type="function", function=tool_function)

    # Construct the OpenAI API payload
    payload = OpenAIRequest(
        model="gpt-4-turbo",
        messages=[
            Message(role="system", content=system_prompt),
            Message(role="user", content=user_message),
        ],
        tools=[tool],
        tool_choice="required",
    )

    ep = os.environ["AZURE_OPENAI_ENDPOINT"]
    api_v = os.environ["AZURE_OPENAI_API_VERSION"]
    url = f"{ep}openai/deployments/{model}/chat/completions?api-version={api_v}"

    async with httpx.AsyncClient() as client:
        response = await client.post(
            url,
            headers={
                "Content-Type": "application/json",
                "Authorization": "Bearer " + os.environ["AZURE_OPENAI_API_KEY"],
            },
            data=payload.model_dump(),
        )
        response_data = response.json()

    # Extract the result from the response
    result_content = response_data["choices"][0]["message"]["content"]
    result_data = json.loads(result_content)

    # Parse the result into the response model
    result = response_model(**result_data)

    return result


class ResumeFree(BaseModel):
    text: str


class ResumeStructured(BaseModel):
    name: str
    email: str
    skills: list[str]


instr = "Extract the necessary entities from the provided short resume text."

free = ResumeFree(
    text="My name is Paul and you can get in touch with me over mail@plpp.de. I'm mostly interested in working in AI projects. My strong suits are Computer Vision and Natural Language Processing"
)

res = asyncio.run(ai_function(instr, free, ResumeStructured))
print(res)
