import json
import base64

import azure.functions as func

from models import Users


def render_template(env, template_name, **kwargs):
    template = env.get_template(template_name)
    return template.render(**kwargs)


def authenticate(req: func.HttpRequest):
    client_header = req.headers.get("X-MS-CLIENT-PRINCIPAL")
    if client_header:
        # Decodes the base64 encoded user data provided by SWA from the header
        client_principal = json.loads(base64.b64decode(client_header).decode("utf-8"))
        if client_principal.get("userDetails"):
            if client_principal.get("userDetails") != "pietz":
                return None
            return Users.from_client_principal(client_principal)
    return None
