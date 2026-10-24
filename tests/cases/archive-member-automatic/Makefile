.SUFFIXES: .o
lib.a(dir/one.o): input | stamp
	@printf "@=[%s] %%=[%s] <=[%s] ^=[%s] +=[%s] ?=[%s] |=[%s] *=[%s]\n" "$@" "$%" "$<" "$^" "$+" "$?" "$|" "$*"
