# Directive whitespace and definition bodies

Top-level directive keywords use the six C-locale whitespace bytes as token
boundaries. Leading vertical tabs and a vertical tab after a keyword are
recognized alongside the existing space, tab, CR and form-feed handling.
Conditional keyword dispatch, modifier parsing and export lists retain this
boundary without changing rule filename tokenization.

Assignment recognition still runs first. For example, `override` followed by a
vertical tab and `X=one` can name one ordinary variable containing that tab.
Adding a space before `X` rejects that raw assignment, allowing modifier parsing
to select the override origin for `X`. Original global and target fixtures check
both interpretations rather than assuming every C separator starts a modifier.

Active `define` bodies have a distinct grammar: leading C whitespace is skipped
for keyword recognition, but nested `define` and closing `endef` require a space
or tab boundary. Other bytes immediately following the keyword remain literal
body content. A recipe-prefix byte protects a body line from nesting recognition.
Extraneous text after a recognized `endef` emits a warning, including nested
closures, without expanding that text. Comments are removed for this check.
Continued closing lines report their final observed source line.

Skipped definitions follow the outer reader's ignored-definition state rather
than active-body nesting. The first standalone `endef`, after ordinary comment
handling and C-whitespace tokenization, resumes conditional reading. Nested
`define` text does not add depth. Names and body expressions have no effects,
extraneous closing text does not warn or close the skipped definition, and EOF
reports the still-open conditional. NUL warnings use the outer read position,
distinct from active definition-body reads.
Recipe-prefix protection in this skipped state applies when a preceding rule is
still open; without that rule, leading prefix bytes can be ordinary whitespace
or part of the `endef` keyword.

The tranche adds 117 original fixtures and promotes the existing export-keyword
vertical-tab case. A 160-program directive/body matrix, a 336-program skipped-body
matrix and the 2,380-program modifier matrix match exact streams and statuses.
An expanded 972-program keyword/operator/C-separator matrix also matches.
General definition header
grammar, qualified definitions, all conditional operand rules and dynamic input
remain separate compatibility work; these results do not establish full parsing
compatibility.

GNU Make 4.4.1 `src/read.c` (`eval`, `parse_var_assignment`, `conditional_line`,
`do_define`) was consulted for token boundaries, active/ignored definition phases
and diagnostic positions. The Rust implementation and fixtures are independently
authored; no upstream source text or tests were copied.
