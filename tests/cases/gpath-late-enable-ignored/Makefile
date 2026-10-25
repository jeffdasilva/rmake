seed := $(shell touch -d @1600000000 src/foo.out input; touch -d @1600000002 input)
VPATH=src
all: change foo.out
change: ; @$(eval GPATH=src)
foo.out: input
	@printf "@=[%s] *=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] V=[%s]\n" "$@" "$*" "$<" "$^" "$+" "$|" "$(V)"
