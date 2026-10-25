seed := $(shell touch -d @1600000000 src/foo.out input extra; touch -d @1600000002 input extra)
VPATH=src
GPATH=src
all: foo.out
foo.out: input
	@printf "@=[%s] *=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] V=[%s]\n" "$@" "$*" "$<" "$^" "$+" "$|" "$(V)"
src/foo.out: extra
