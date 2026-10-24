all: lib.a(one.o)
(%): %
	@printf "wrong\n"
%:
	@printf "@=[%s] %%=[%s] <=[%s] *=[%s]\n" "$@" "$%" "$<" "$*"
