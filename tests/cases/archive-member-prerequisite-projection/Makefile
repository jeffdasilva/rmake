all: one.a(same.o) two.a(same.o) one.a(same.o) | three.a(other.o)
	@printf "@=[%s] %%=[%s] <=[%s] ^=[%s] +=[%s] ?=[%s] |=[%s] *=[%s]\n" "$@" "$%" "$<" "$^" "$+" "$?" "$|" "$*"
