all: lib.a(one.o)
lib.a(%.o): %.middle
	@printf "@=[%s] %%=[%s] <=[%s] *=[%s]\n" "$@" "$%" "$<" "$*"
%.middle: %.src;@cp $< $@
(%.o): %.src
	@printf "wrong\n"
