seed := $(shell touch -d @1600000000 src/foo.out input foo.in; touch -d @1600000002 input foo.in)
VPATH=src
GPATH=src
all: foo.out
foo%.out: %.in
	@echo wrong
src/%.out: %.in
	@printf "@=[%s] *=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] V=[%s]\n" "$@" "$*" "$<" "$^" "$+" "$|" "$(V)"
