all: lib.a(one.o)
(%.o):: %.src
	@printf "@=[%s] %%=[%s] <=[%s] *=[%s]\n" "$@" "$%" "$<" "$*"
