seed := $(shell touch -d @1600000000 src/foo.out; touch -d @1600000002 input)
VPATH=src
COUNT:=
foo.out: input
	@echo unused=$@
src/foo.out: input
	-@python3 build.py $(eval COUNT+=x)$(words $(COUNT)) "$@" "$(NEW)" ignored
	@python3 after.py $(words $(COUNT)) "$(NEW)"
