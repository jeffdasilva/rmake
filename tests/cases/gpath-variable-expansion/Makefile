seed := $(shell touch -d @1600000000 src/foo.out input; touch -d @1600000002 input)
WHERE=src
VPATH=$(WHERE)
GPATH=$(WHERE)
all: foo.out
foo.out: input
	@printf "@=[%s] *=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] V=[%s]\n" "$@" "$*" "$<" "$^" "$+" "$|" "$(V)"
