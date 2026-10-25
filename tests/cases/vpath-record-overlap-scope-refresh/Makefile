seed := $(shell touch -d @1600000000 src/foo.out; touch -d @1600000002 input)
VPATH=src
COUNT:=
foo.out: NEW=from-original
src/foo.out: OLD=actual
foo.out: input
	@echo unused=$@
src/foo.out: input
	@python3 build.py $(eval COUNT+=x)$(words $(COUNT)) "$@" "$(NEW)" normal
	@python3 after.py $(words $(COUNT)) "$(NEW)"
