import pytest
from pydantic import BaseModel
from ai import ai_function, model2credits
import os
from dotenv import load_dotenv

load_dotenv()

class TestInput(BaseModel):
    name: str
    age: int

class TestOutput(BaseModel):
    greeting: str
    birth_year: int

@pytest.fixture
def test_instructions():
    return "Given a name and age, return a greeting and the person's birth year."

@pytest.mark.asyncio
async def test_ai_function():
    instructions = "Given a name and age, return a greeting and the person's birth year."
    inputs = TestInput(name="Alice", age=30)
    model = "gpt-35-turbo"
    
    result = await ai_function(instructions, inputs, TestOutput, model)
    
    assert isinstance(result, TestOutput)
    assert "Alice" in result.greeting
    assert 1990 <= result.birth_year <= 1993  # Allowing some flexibility for the AI's response

def test_model2credits():
    assert "gpt-35-turbo" in model2credits
    assert "gpt-4" in model2credits
    assert model2credits["gpt-35-turbo"] == 500
    assert model2credits["gpt-4"] == 50

@pytest.mark.asyncio
async def test_ai_function_with_different_models():
    instructions = "Given a name and age, return a greeting and the person's birth year."
    inputs = TestInput(name="Bob", age=25)
    
    for model in ["gpt-35-turbo", "gpt-4"]:
        result = await ai_function(instructions, inputs, TestOutput, model)
        assert isinstance(result, TestOutput)
        assert "Bob" in result.greeting
        assert 1995 <= result.birth_year <= 1998

@pytest.mark.asyncio
async def test_ai_function_error_handling():
    instructions = "Invalid instructions"
    inputs = TestInput(name="Charlie", age=40)
    model = "gpt-35-turbo"
    
    with pytest.raises(Exception):
        await ai_function(instructions, inputs, TestOutput, model)

def test_environment_variables():
    required_vars = [
        "MARVIN_AZURE_OPENAI_API_KEY",
        "MARVIN_AZURE_OPENAI_ENDPOINT",
        "MARVIN_AZURE_OPENAI_API_VERSION",
        "MARVIN_CHAT_COMPLETIONS_MODEL"
    ]
    
    for var in required_vars:
        assert var in os.environ, f"{var} is not set in the environment"

@pytest.mark.asyncio
async def test_ai_function_integration():
    instructions = "Given a name and age, return a greeting and the person's birth year."
    inputs = TestInput(name="David", age=35)
    model = "gpt-35-turbo"
    
    result = await ai_function(instructions, inputs, TestOutput, model)
    
    assert isinstance(result, TestOutput)
    assert "David" in result.greeting
    assert 1985 <= result.birth_year <= 1988
    
    # Test credit consumption
    chars = len(instructions) + len(inputs.model_dump_json()) + len(result.model_dump_json())
    credits_used = chars // model2credits[model] + (1 if chars % model2credits[model] > 0 else 0)
    assert credits_used > 0

if __name__ == "__main__":
    pytest.main()
