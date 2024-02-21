import os
from jinja2 import Environment, FileSystemLoader


def render(env, inp, out):
    template = env.get_template(inp)
    output = template.render()
    with open(out, "w") as f:
        f.write(output)


env = Environment(loader=FileSystemLoader("api/templates"))

render(env, "landing.html", "public/index.html")
render(env, "login.html", "public/login/index.html")
# render(env, "dashboard.html", "public/app/index.html")
