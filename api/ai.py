import os
from typing import Any
from pydantic import BaseModel
import marvin
from marvin.ai.text import _generate_typed_llm_response_with_tool
from marvin.ai.prompts.text_prompts import FUNCTION_PROMPT

marvin.settings.provider = "azure_openai"
marvin.settings.azure_openai_api_key = os.environ["MARVIN_AZURE_OPENAI_API_KEY"]
marvin.settings.azure_openai_endpoint = os.environ["MARVIN_AZURE_OPENAI_ENDPOINT"]
marvin.settings.azure_openai_api_version = os.environ["MARVIN_AZURE_OPENAI_API_VERSION"]
marvin.settings.openai.chat.completions.model = os.environ[
    "MARVIN_CHAT_COMPLETIONS_MODEL"
]

model2credits = {
    # number of chars per credit per model
    "gpt-35-turbo": 500,
    "gpt-4": 50,
}


async def ai_function(
    instructions: str, inputs: BaseModel, output_type: Any, model: str
):
    return await _generate_typed_llm_response_with_tool(
        prompt_template=FUNCTION_PROMPT,
        prompt_kwargs=dict(
            fn_definition=instructions,
            bound_parameters=inputs.model_dump(),
            return_value=str(output_type),
        ),
        model_kwargs={"model": model},
        type_=output_type,
    )
