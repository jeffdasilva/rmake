all: dir/lib.a(one.o)
dir/lib.a(%.o): %.src
	@printf "@=[%s] %%=[%s] <=[%s] *=[%s]\n" "$@" "$%" "$<" "$*"
