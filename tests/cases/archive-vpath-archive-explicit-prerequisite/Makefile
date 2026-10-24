vpath %.a store
all: lib.a(one.o)
lib.a(one.o): lib.a
	@printf "@=[%s] %%=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] *=[%s] V=[%s]\n" "$@" "$%" "$<" "$^" "$+" "$|" "$*" "$(V)"
lib.a:;@printf "archive=[%s]\n" "$@"
