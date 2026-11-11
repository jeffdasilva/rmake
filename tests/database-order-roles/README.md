# Prerequisite roles in the database

GNU Make can change duplicate prerequisite roles when it prepares an active
recipe. The position of the first normal occurrence matters. The database
must show the actual branch's retained roles. An unprepared rule keeps its
original declarations. Automatic-variable effects still use their proper
values at the time they were prepared.

`make test-database-order-roles` runs nine original cases against GNU Make
4.4.1 twice and the selected rmake binary. It checks the entire database
transcript with two independently bounded clock fields, literal prerequisite
lines, status, stderr, automatic-variable witnesses, and the full filesystem.
Both build modes pass all nine cases; the preceding build passed three. The
cases include inactive targets, reverse occurrence order, late eval,
double-colon branch scope, question, dry-run, and touch modes.

Sixteen additional cases compare secondary prerequisite expansion in the
database. The active rule prints the realized names and its retained
`.WAIT` and order-only partitions. An unvisited target or double-colon
branch still prints its deferred text. Controls cover multiple deferred
clauses, late `-p`, reverse shuffle, question, dry-run and touch.
