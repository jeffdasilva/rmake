# How rmake chooses a build rule

rmake starts with the rules for the requested target. If those rules supply
prerequisites but no recipe, rmake can look for an implicit rule. A pattern
rule such as `%.txt: %.raw` describes how to build any matching target from
an input with the same stem. For the syntax and full selection algorithm, see
[implicit rule search in the GNU Make manual](https://www.gnu.org/software/make/manual/html_node/Implicit-Rule-Search.html).

## Choose the default goal

When you run `rmake` without a goal, it starts with the Makefile's default
goal. Usually this is the first eligible target. Set `.DEFAULT_GOAL := all` to
name it explicitly, or pass a goal on the command line to select a different
target. Pattern rules such as `%.txt: %.raw` never select the default goal.
See [special variables in GNU Make](https://www.gnu.org/software/make/manual/html_node/Special-Variables.html).

## Try a chain of pattern rules

In an empty directory, save this example as `Makefile`. Start each recipe line
with a tab.

```makefile
.PHONY: all
all: report.txt

%.txt: %.mid
	@cp "$<" "$@"

%.mid: %.raw
	@cp "$<" "$@"
```

1. Create the input and build the default goal.

   ```sh
   printf '%s\n' 'Example report' > report.raw
   rmake -rR
   ```

   The `-rR` options keep the example focused on the rules in this Makefile.
   The build creates `report.mid`, copies it to `report.txt`, and removes
   `report.mid` as an intermediate file.

2. Run `rmake -rR` again. The report is up to date and the recipes are skipped.

To see how rmake tracks which intermediates to remove, read
[intermediate-file cleanup](file-record-order.md).

## Diagnose rule selection

- When several patterns match, rmake considers the stem length and declaration
  order. A later declaration with identical targets and prerequisites can replace
  or cancel an earlier recipe. To see the attempts, use `rmake --debug=i`.
- A target pattern that contains a slash matches the full path. A pattern
  without a slash matches the filename portion. For `virtual/box/` and
  `b%`, the filename stem is `ox/` and the reconstructed stem is
  `virtual/ox/`. The reconstructed stem matters when substituting prerequisites.
- With [static pattern rules](https://www.gnu.org/software/make/manual/html_node/Static-Pattern.html),
  each target has its own stem and keeps its order-only prerequisites. If
  a target doesn't match the pattern, rmake warns and retains the recipe.
- [Double-colon rules](https://www.gnu.org/software/make/manual/html_node/Double_002dColon.html)
  let multiple rules update a target independently. Leave automatic remakes of
  Makefiles to rules that have prerequisites. An unconditional double-colon
  Makefile recipe is skipped during remaking to avoid a restart loop.
- [Secondary expansion](https://www.gnu.org/software/make/manual/html_node/Secondary-Expansion.html)
  expands deferred prerequisite expressions when rmake considers the target.
  The expansion can read target-specific and applicable automatic variables.
  A clause with a recipe expands after earlier clauses. Its prerequisites
  are placed first for execution.
- `VPATH` and `vpath` add places to search for files. Search results can affect
  names in automatic variables. A rebuilt target normally uses its local path.
  See GNU Make's [directory search guide](https://www.gnu.org/software/make/manual/html_node/Directory-Search.html)
  and rmake's [GPATH tests](gpath.md).

Run `make test-diff` from the rmake checkout to compare the curated cases.
Use the [M4 audit](m4-exit-audit.md) for the acceptance scope and the
[M3/M6 audit](m3-m6-audit.md) for open combinations involving rule selection,
options, debug output, and file observations.
