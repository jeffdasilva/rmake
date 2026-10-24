VPATH=store
all: first second
first: lib.a(one.o);@cp replacement.a lib.a
second: lib.a(two.o)
	@printf "@=[%s] %%=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] *=[%s] V=[%s]\n" "$@" "$%" "$<" "$^" "$+" "$|" "$*" "$(V)"
