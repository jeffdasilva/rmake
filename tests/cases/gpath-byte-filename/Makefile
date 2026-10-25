seed := $(shell touch -d @1600000000 src/foo.out input 'src/ÿ.out'; touch -d @1600000002 input)
VPATH=src
GPATH=src
all: ÿ.out
ÿ.out: input
	@printf "@=[%s] *=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] V=[%s]\n" "$@" "$*" "$<" "$^" "$+" "$|" "$(V)"
