import logging
import json
import base64
import azure.functions as func

app = func.FunctionApp(http_auth_level=func.AuthLevel.FUNCTION)

def authenticate(req: func.HttpRequest):
    client_header = req.headers.get("X-MS-CLIENT-PRINCIPAL")
    if client_header:
        client_principal = json.loads(
            base64.b64decode(client_header).decode("utf-8")
        )
        if client_principal.get("userDetails"):
            return client_principal
        else:
            return func.HttpResponse("User is not authenticated", status_code=401)
    else:
        return func.HttpResponse(
            "Authentication header is missing", status_code=401
        )


@app.route(route="health", methods=["GET"])
def health(req: func.HttpRequest) -> func.HttpResponse:
    return func.HttpResponse("OK", status_code=200)


@app.route(route="app", methods=["GET"])
def dashboard(req: func.HttpRequest) -> func.HttpResponse:
    result = authenticate(req)
    if isinstance(result, func.HttpResponse):
        # not authenticated
        return result
    print(result)
    return func.HttpResponse("OK", status_code=200)
