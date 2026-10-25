seed := $(shell touch -d @1600000000 src/foo.out input; touch -d @1600000002 input)
VPATH=src
GPATH=src
all: foo.out
foo%: V=original
src/%: V=resolved
foo.out: input
	@printf "@=[%s] *=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] V=[%s]\n" "$@" "$*" "$<" "$^" "$+" "$|" "$(V)"
