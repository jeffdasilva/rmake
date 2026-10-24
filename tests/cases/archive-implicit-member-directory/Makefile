all: lib.a(dir/one.o)
(%.o): %.src
	@printf "@=[%s] %%=[%s] <=[%s] *=[%s]\n" "$@" "$%" "$<" "$*"
