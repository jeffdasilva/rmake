$(shell touch -d @1600000001 input)
lib.a(one.o): input
	@printf "@=[%s] %%=[%s] <=[%s] ^=[%s] +=[%s] ?=[%s] |=[%s] *=[%s]\n" "$@" "$%" "$<" "$^" "$+" "$?" "$|" "$*"
