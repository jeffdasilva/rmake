.SUFFIXES: .src .a
all: lib.a(one.o)
.src.a:;@printf "wrong\n"
(%.o): %.src
	@printf "@=[%s] %%=[%s] <=[%s] *=[%s]\n" "$@" "$%" "$<" "$*"
