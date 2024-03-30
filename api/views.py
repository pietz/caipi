from htpy import body, header, main, footer, section, nav
from htpy import article, dialog, button, ul, li, div, img
from htpy import form, input, label, textarea, select, option
from htpy import h1, h2, h3, h4, h5, h6, p, br, strong
from htpy import table, thead, tbody, tr, th, td

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
                h5["Create a new endpoint"],
            ],
            form(hx_post="/api/projects", hx_target="body")[
                label[strong["Endpoint Name"]],
                section(".grid")[
                    input(name="name", placeholder="Translator", required=True)
                ],
                label["Instructions"],
                section(".grid")[
                    textarea(
                        name="instruction",
                        placeholder="Translate the input text to...",
                        required=True,
                    ),
                ],
                label["Request"],
                section(".grid")[
                    input(name="reqname", value="input", required=True),
                    input(name="reqdefault", placeholder="Default Value"),
                    select(name="reqtype")[
                        option(selected=True)["Text"],
                        option["Number"],
                        option["Boolean"],
                    ],
                ],
                # section(".grid")[
                #     *[
                #         (
                #             input(name=f"reqname{i+1}", value="input", required=True),
                #             input(name=f"reqdefault{i+1}", placeholder="Default Value"),
                #             select(name=f"reqdtype{i+1}")[
                #                 option(selected=True)["Text"],
                #                 option["Number"],
                #                 option["Boolean"],
                #             ],
                #         )
                #         for i in range(n_req)
                #     ]
                # ],
                label["Response"],
                section(".grid")[
                    input(name="resname", value="output", required=True),
                    input(name="resdefault", placeholder="Default Value"),
                    select(name="restype")[
                        option(selected=True)["Text"],
                        option["Number"],
                        option["Boolean"],
                    ],
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


def dashboard(user: Users, projects: list[Projects]):
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
            project_table(projects),
        ],
        modal(),
    ]


def navigation():
    return (
        header(".container-fluid", style="box-shadow: var(--pico-box-shadow)")[
            nav[ul[li["caipi"]], ul[(li["Link"] for _ in range(4))]],
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
        article(".card")[
            table[
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
            table[
                thead[tr[(th[x] for x in cols)]],
                tbody[(table_row(x) for x in projects)],
            ]
        ],
    ]


def tiles(invocations: int, chars: int, latency: float, success: float):
    return section(".grid")[
        article(".card")[h6["Invocations"], h3[str(invocations)]],
        article(".card")[h6["Credits"], h3[str(chars)]],
        article(".card")[h6["Latency"], h3[str(round(latency, 1)) + "s"]],
        article(".card")[h6["Success"], h3[str(int(success * 100)) + "%"]],
    ]
