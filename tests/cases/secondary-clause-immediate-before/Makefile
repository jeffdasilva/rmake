.SECONDEXPANSION:
all: one two
all: $$(info delayed @=[$$@] *=[$$*] <=[$$<] ^=[$$^] +=[$$+] |=[$$|] ?=[$$?])three
	@printf "@=[%s] *=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] ?=[%s]\n" "$@" "$*" "$<" "$^" "$+" "$|" "$?"
one two three four keep order item.src tem.src item.a tem.b dollar$$X: ;
