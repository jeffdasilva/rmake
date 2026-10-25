seed := $(shell touch -d @1600000000 src/foo.out input; touch -d @1599999998 input)
VPATH=src
GPATH=src
foo.out: input
	@printf "@=[%s] *=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] V=[%s]\n" "$@" "$*" "$<" "$^" "$+" "$|" "$(V)"
