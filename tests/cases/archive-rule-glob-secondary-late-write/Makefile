.SECONDEXPANSION:
all: $$(shell cp source.a lib.a) lib.a($$(PATTERN))
	@printf "@=[%s] %%=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] *=[%s] V=[%s]\n" "$@" "$%" "$<" "$^" "$+" "$|" "$*" "$(V)"
PATTERN=*.o
