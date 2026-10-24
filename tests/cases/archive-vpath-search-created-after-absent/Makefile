VPATH=store
all: first second
first:;@cp replacement.a store/lib.a
second: lib.a(one.o)
	@printf "@=[%s] %%=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] *=[%s] V=[%s]\n" "$@" "$%" "$<" "$^" "$+" "$|" "$*" "$(V)"
