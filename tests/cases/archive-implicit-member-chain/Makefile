all: lib.a(one.o)
(%): %
	@printf "@=[%s] %%=[%s] <=[%s] *=[%s]\n" "$@" "$%" "$<" "$*"
%.o: %.src;@cp $< $@
