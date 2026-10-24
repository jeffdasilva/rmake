.SECONDEXPANSION:
item.o: %.o: $$(info A @=[$$@] *=[$$*] <=[$$<] ^=[$$^] +=[$$+] |=[$$|] ?=[$$?])one
	@printf "@=[%s] *=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] ?=[%s]\n" "$@" "$*" "$<" "$^" "$+" "$|" "$?"
item.o: i%.o: $$(info B @=[$$@] *=[$$*] <=[$$<] ^=[$$^] +=[$$+] |=[$$|] ?=[$$?])two
item.o: it%.o: $$(info C @=[$$@] *=[$$*] <=[$$<] ^=[$$^] +=[$$+] |=[$$|] ?=[$$?])three
one two three four keep order item.src tem.src item.a tem.b dollar$$X: ;
