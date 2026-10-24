.SUFFIXES: .o .a
all: lib.a(one.o)
.o.a:
	@printf "@=[%s] %%=[%s] <=[%s] *=[%s]\n" "$@" "$%" "$<" "$*"
