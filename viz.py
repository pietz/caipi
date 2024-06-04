import pygal

from models import Invocations

def invocation_chart(invocations: list[Invocations]) -> str:
    # Process data for Pygal
    invocation_counts = {}
    for invocation in invocations:
        project = invocation.project
        date = invocation.timestamp.strftime("%Y-%m-%d")

        if project not in invocation_counts:
            invocation_counts[project] = {}
        if date not in invocation_counts[project]:
            invocation_counts[project][date] = 0

        invocation_counts[project][date] += 1

    style = pygal.style.Style(
        background="transparent",
        plot_background="transparent",
        colors=("#000000", "#A9D80D", "#439C3A", "#122E38", "#DEEBE1"),
    )

    # Create Pygal Area chart
    area_chart = pygal.StackedLine(
        fill=True,
        height=200,
        width=960,
        interpolate="cubic",
        show_legend=False,
        x_label_rotation=0,
        show_minor_y_labels=False,
        style=style,
    )
    area_chart.x_labels = sorted(
        {date for project_data in invocation_counts.values() for date in project_data}
    )

    for project, counts in invocation_counts.items():
        values = [counts.get(date, 0) for date in area_chart.x_labels]
        area_chart.add(project, values)

    # Render the chart to an SVG string
    return area_chart.render(is_unicode=True)