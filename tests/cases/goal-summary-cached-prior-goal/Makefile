.PHONY: first
first: ; @printf 'first\n'
later: | first
later: ; @$(error cached prerequisite should leave later current)
