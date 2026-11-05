# Recipe launch at low descriptor limits

Ten cases exercise recipe launch with descriptor limits from 4 through 7.
They check successful execution, actual execution errors, a program that
returns 127, ordered exported entries, inherited SIGPIPE disposition, and
PATH search through an executable without a shebang. That last case measures
the system's `/bin/sh` in a separate control because shells differ in their
ability to open script files at limit 4. The gate checks exact output and status
against that control, including make's recipe failure message where applicable.

Two live limit-4 cases verify SIGTERM and SIGINT. Each recipe writes an owned
target. The controller verifies the live process, handler, descriptor limits
and wake state, then requires make to remove the target before it releases
the recipe. The complete filesystem and process containment are checked.

Run `tools/resource_fd_recipes.py --candidate PATH --report PATH`, or use
`--oracle-only`. The two reference executions use the verified GNU oracle.
`recipe-fdlimit-4` is the focused recipe case. The gate reuses the startup
and cancellation driver from `tools/resource_fd.py`.
