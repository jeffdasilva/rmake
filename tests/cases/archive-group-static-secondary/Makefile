.SECONDEXPANSION:
all: lib.a(one.o two.o)
lib.a(one.o two.o): lib.a(%.o): $$(addsuffix .src,$$*)
	@printf "@=[%s] %%=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] *=[%s] V=[%s]\n" "$@" "$%" "$<" "$^" "$+" "$|" "$*" "$(V)"
