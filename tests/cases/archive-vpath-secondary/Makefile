.SECONDEXPANSION:
vpath %.a store
DEPS=lib.a(one.o two.o)
all: $$(DEPS)
	@printf "@=[%s] %%=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] *=[%s] V=[%s]\n" "$@" "$%" "$<" "$^" "$+" "$|" "$*" "$(V)"
