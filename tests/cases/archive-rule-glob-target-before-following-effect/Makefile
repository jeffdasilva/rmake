all: lib.a(one.o two.o)
lib.a(*.o):
	@printf "@=[%s] %%=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] *=[%s] V=[%s]\n" "$@" "$%" "$<" "$^" "$+" "$|" "$*" "$(V)"
X := $(shell cp source.a lib.a)
