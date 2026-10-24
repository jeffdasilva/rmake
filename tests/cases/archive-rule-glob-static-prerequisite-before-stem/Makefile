one.out: %.out: lib.a(%.o*)
	@printf "@=[%s] %%=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] *=[%s] V=[%s]\n" "$@" "$%" "$<" "$^" "$+" "$|" "$*" "$(V)"
lib.a(%.o*):;@printf "member=[%s]\n" "$%"
