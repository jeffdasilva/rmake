all: lib.a(one.o two.o)
lib.a(*.o): V=matched
lib.a(one.o two.o):
	@printf "@=[%s] %%=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] *=[%s] V=[%s]\n" "$@" "$%" "$<" "$^" "$+" "$|" "$*" "$(V)"
