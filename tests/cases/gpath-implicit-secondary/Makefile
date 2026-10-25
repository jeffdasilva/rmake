seed := $(shell touch -d @1600000000 src/foo.out input src/foo.in; touch -d @1600000002 input src/foo.in)
.SECONDEXPANSION:
VPATH=src
GPATH=src
all: foo.out
%.out: $$(info CANDIDATE=$$@ STEM=$$*)%.in
	@printf "@=[%s] *=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] V=[%s]\n" "$@" "$*" "$<" "$^" "$+" "$|" "$(V)"
