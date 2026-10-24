all: first second
first: lib.a(*.o)
	@printf "@=[%s] %%=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] *=[%s] V=[%s]\n" "$@" "$%" "$<" "$^" "$+" "$|" "$*" "$(V)"
$(file >lib.a,invalid)
second: lib.a(*.o)
.DEFAULT:;@printf "%s:%s\n" "$@" "$%"
