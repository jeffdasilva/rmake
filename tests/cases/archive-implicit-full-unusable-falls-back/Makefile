all: lib.a(one.o)
lib.a(%.o): %.absent
	@printf "wrong\n"
(%.o): %.src
	@printf "@=[%s] %%=[%s] <=[%s] *=[%s]\n" "$@" "$%" "$<" "$*"
