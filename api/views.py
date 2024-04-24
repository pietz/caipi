from htpy import body, header, main, footer, section, nav, details
from htpy import article, dialog, button, ul, li, div, img, summary
from htpy import form, input, label, textarea, select, option, fieldset
from htpy import h1, h2, h3, h4, h5, h6, p, br, strong, a, progress
from htpy import table, thead, tbody, tr, th, td, hgroup, small

from models import Invocations, Users, Projects


def modal(proj: Projects | None = None):
    return dialog("#modal", open=False)[
        article[
            header[
                button(
                    aria_label="Close",
                    rel="prev",
                    onclick="modal.close()",
                ),
                hgroup[
                    h4(style="padding: 6px 0 6px 0")["Create a new endpoint"],
                    p["Define a new endpoint to be used by the API"],
                ],
            ],
            form(
                hx_post="/api/projects" if not proj else None,
                hx_patch=f"/api/projects/{proj.id}" if proj else None,
                hx_target="main",
                hx_swap="outerHTML",
            )[
                div[
                    label[strong["Endpoint Name"]],
                    input(name="name", value=proj.name if proj else ""),
                    label[strong["Instructions"]],
                    textarea(name="instructions")[proj.instructions if proj else ""],
                ],
                section(".grid")[
                    payload_params("Request", data=proj.request if proj else None),
                    payload_params("Response", data=proj.response if proj else None),
                ],
                details[
                    summary(".outline.contrast", role="button")["Advanced Settings"],
                    div(".grid")[
                        div[
                            label["Model"],
                            select(aria_label="Model")[
                                option["gpt-35-turbo"],
                                option["gpt-4"],
                            ],
                            label[
                                input(type="checkbox", role="switch", disabled=True),
                                "Collect Payload Data in Invocations (Plus)",
                            ],
                            label[
                                input(type="checkbox", role="switch"),
                                "Include HTTP Status Code in Response",
                            ],
                        ],
                        div[
                            label["Temperature"],
                            input(
                                type="range", value="0", min="0", max="1", step="0.1"
                            ),
                            label[
                                input(type="checkbox", role="switch", disabled=True),
                                "Validate Response Quality with AI ",
                                strong[small["Pro"]],
                            ],
                        ],
                    ],
                ],
                input(
                    ".contrast",
                    type="submit",
                    value="Create Endpoint" if proj is None else "Save Changes",
                    onclick="modal.close()",
                ),
            ],
        ]
    ]


def param(prefix: str = "req", value: str = "", dtype: str = "str"):
    j2p = {"Text": "str", "Number": "float", "Boolean": "bool"}
    if dtype in j2p:
        dtype = j2p[dtype]
    opts = ["str", "int", "float", "bool"]
    return fieldset(role="group")[
        input(name=f"{prefix}_name", value=value, required=True),
        select(name=f"{prefix}_dtype")[(option(selected=x == dtype)[x] for x in opts)],
    ]


def payload_params(type: str = "Request", data: dict | None = None):
    div_id = "#" + type.lower()
    abr = type[:3].lower()
    if data is None:
        form_el = param(abr, value="input" if abr == "req" else "output")
    else:
        form_el = (param(abr, value=k, dtype=v[0]) for k, v in data.items())
    return div[
        label[strong[type]],
        div(div_id)[form_el],
        fieldset(role="group")[
            button(
                ".outline.secondary",
                hx_get=f"/api/modal/add/{abr}",
                hx_target=div_id,
                hx_swap="beforeend",
            )["Add Parameter"]
        ],
    ]


def dashboard(user: Users, projects: list[Projects]):
    return main(".container", style="margin-top: 24px")[
        br,
        section(style="display:flex;justify-content:space-between")[
            h1["Dashboard"],
            div[button(".contrast", onclick="modal.showModal()")["Create Endpoint"]],
        ],
        tiles(
            invocations=user.invocations,
            credits=user.credits_avail - user.credits_used,
            latency=user.latency,
            success=user.success,
        ),
        project_table(projects),
        modal(),
    ]


def project_page(project: Projects, invocations: list[Invocations]):
    return main(".container", style="margin-top: 24px")[
        br,
        section(style="display:flex;justify-content:space-between")[
            h1[project.name],
            div[button(".contrast", onclick="modal.showModal()")["Edit Endpoint"]],
        ],
        tiles(
            invocations=project.invocations,
            credits=project.credits,
            latency=project.latency,
            success=project.success,
        ),
        invocation_table(invocations),
        modal(project),
    ]


def table_row(project: Projects):
    return tr[
        td[
            a(
                ".contrast",
                href=f"/app/{project.id}",
                hx_get=f"/api/app/{project.id}",
                hx_push_url=f"/app/{project.id}",
                hx_swap="outerHTML",
                hx_target="main",
            )[project.name]
        ],
        td["/" + project.endpoint],
        td[str(project.invocations)],
        td[str(project.credits)],
        td[str(round(project.latency, 1)) + "s"],
        td[str(int(project.success * 100)) + "%"],
    ]


def project_table(projects: list[Projects]):
    cols = [
        "Name",
        "URL",
        "Invocations",
        "Credits",
        "Latency",
        "Success",
    ]
    return section("#table")[
        article(style="padding:7px 0 0 0")[
            table(".striped")[
                thead[tr[(th[x] for x in cols)]],
                tbody[(table_row(x) for x in projects)],
            ]
        ],
    ]


def invocation_row(invocation: Invocations):
    return tr[
        td[invocation.id],
        td[
            str(invocation.timestamp.replace(tzinfo=None))
            if invocation.timestamp
            else "???"
        ],
        td[str(invocation.credits)],
        td[str(round(invocation.latency, 1)) + "s"],
        td[str(invocation.success)],
    ]


def invocation_table(invocations: list[Invocations]):
    cols = [
        "ID",
        "TS (UTC)",
        "Credits",
        "Latency",
        "Success",
    ]
    return section("#table")[
        article(style="padding:7px 0 0 0")[
            table(".striped")[
                thead[tr[(th[x] for x in cols)]],
                tbody[(invocation_row(x) for x in invocations)],
            ]
        ],
    ]


def tiles(invocations: int, credits: int, latency: float, success: float):
    return section(".grid")[
        article[h6["Invocations"], h3[str(invocations)]],
        article[h6["Credits"], h3[str(credits)]],
        article[h6["Latency"], h3[str(round(latency, 1)) + "s"]],
        article[h6["Success"], h3[str(int(success * 100)) + "%"]],
    ]


def tile(title: str, value: str, prog: tuple | None):
    return article[
        hgroup[
            h6[title],
            h3[value],
            prog and progress(value=prog[0], max=prog[1]),
        ]
    ]


def selectx(name: str, options=["One", "Two", "Three"], required=False):
    return select(name=name, aria_label="Select", required=required)[
        option(selected=True)[options[0]], (option[x] for x in options[1:])
    ]
