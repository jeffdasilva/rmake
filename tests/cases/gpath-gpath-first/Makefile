seed := $(shell touch -d @1600000000 src/foo.out input first/foo.out; touch -d @1600000002 input)
VPATH=src:first
GPATH=first:src
all: foo.out
foo.out: input
	@printf "@=[%s] *=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] V=[%s]\n" "$@" "$*" "$<" "$^" "$+" "$|" "$(V)"
