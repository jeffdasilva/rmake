all: lib.a(one.o)
(%.o): %.src
	@printf "member\n"
lib.a(%.o): %.src
	@printf "@=[%s] %%=[%s] <=[%s] *=[%s]\n" "$@" "$%" "$<" "$*"
