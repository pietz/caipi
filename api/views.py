from htpy import body, header, main, footer, section, nav, details
from htpy import article, dialog, button, ul, li, div, img, summary
from htpy import form, input, label, textarea, select, option, fieldset
from htpy import h1, h2, h3, h4, h5, h6, p, br, strong, a, progress
from htpy import table, thead, tbody, tr, th, td, hgroup

from models import Invocations, Users, Projects


def modal(open: bool = False, n_req: int = 1, n_res: int = 1):
    return dialog("#modal", open=open)[
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
                ]
            ],
            form(hx_post="/api/projects", hx_target="body")[
                div[
                    label[strong["Endpoint Name"]],
                    input(name="title"),
                    label[strong["Instructions"]],
                    textarea(name="instructions"),
                ],
                section(".grid")[
                    payload_params(1, "Request"),
                    payload_params(1, "Response")
                ],
                details[
                    summary(".outline.contrast", role="button")["Advanced Settings"],
                    p["Some more options can go here."],
                ],
                input(
                    ".contrast",
                    type="submit",
                    value="Create Endpoint",
                    onclick="modal.close()",
                ),
            ],
        ]
    ]

def param(prefix: str = "req", value: str = "", disabled: bool = True):
    return fieldset(role="group")[
        input(name=f"{prefix}name", value=value, required=True),
        select(name=f"{prefix}dtype")[
            option(selected=True)["Text"],
            option["Number"],
            option["Boolean"],
        ],
        button(".contrast", disabled=disabled, type="button", hx_get=f"/api/modal/remove/{prefix}", hx_target="closest fieldset", hx_swap="outerHTML")["-"]
    ]

def payload_params(n: int = 1, type: str = "Request"):
    div_id = "#" + type.lower()
    abr = type[:3].lower()
    return div[
        label[strong[type]],
        div(div_id)[param(abr, value="input" if abr == "req" else "output")],
        fieldset(role="group")[
            # button(".outline.secondary")["-"],
            button(".outline.secondary", hx_get=f"/api/modal/add/{abr}", hx_target=div_id, hx_swap="beforeend")["Add Parameter"]
        ]
    ]


def dashboard(user: Users, projects: list[Projects]):
    return div[
        navigation(),
        main(".container", style="margin-top: 24px")[
            br,
            section(style="display:flex;justify-content:space-between")[
                h1["Dashboard"],
                div[
                    button(".contrast", onclick="modal.showModal()")["Create Endpoint"]
                ],
            ],
            tiles(
                invocations=user.invocations,
                chars=user.chars,
                latency=user.latency,
                success=user.success,
            ),
            project_table(projects),
        ],
        modal(),
    ]


def project_page(project: Projects, invocations: list[Invocations]):
    return div[
        navigation(),
        main(".container")[
            br,
            section(style="display:flex;justify-content:space-between")[
                h1["Dashboard"],
                div[
                    button(".contrast", onclick="modal.showModal()")["Create Endpoint"]
                ],
            ],
            tiles(
                invocations=project.invocations,
                chars=project.chars,
                latency=project.latency,
                success=project.success,
            ),
            project_table(project),
        ],
        modal(),
    ]


def navigation():
    return (
        header(".container-fluid", style="box-shadow: var(--pico-box-shadow)")[
            nav[
                ul[
                    li[img(src="/img/caipi.svg", style="width: 36px")],
                    li[strong["caipi.ai"]],
                ],
                ul[li[a(".contrast", href="/logout")["Log out"]]],
            ],
        ],
    )


def table_row(row: Projects):
    return tr[
        td[strong[row.name]],
        td["/" + row.endpoint],
        td[str(row.invocations)],
        td[str(row.chars)],
        td[str(round(row.latency, 1)) + "s"],
        td[str(int(row.success * 100)) + "%"],
    ]


def project_table(projects: list[Invocations]):
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


def invocation_table(projects: list[Projects]):
    cols = [
        "ID",
        "TS",
        "Credits",
        "Latency",
        "Success",
    ]
    return section("#table")[
        article(".card")[
            table(".striped")[
                thead[tr[(th[x] for x in cols)]],
                tbody[(table_row(x) for x in projects)],
            ]
        ],
    ]


def tiles(invocations: int, chars: int, latency: float, success: float):
    return section(".grid")[
        article[h6["Invocations"], h3[str(invocations)]],
        article[h6["Credits"], h3[str(chars)]],
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
