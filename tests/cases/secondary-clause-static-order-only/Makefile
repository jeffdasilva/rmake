.SECONDEXPANSION:
item.o: %.o: $$(info A @=[$$@] *=[$$*] <=[$$<] ^=[$$^] +=[$$+] |=[$$|] ?=[$$?])%.a | one
item.o: i%.o: $$(info B @=[$$@] *=[$$*] <=[$$<] ^=[$$^] +=[$$+] |=[$$|] ?=[$$?])%.b | two
	@printf "@=[%s] *=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] ?=[%s]\n" "$@" "$*" "$<" "$^" "$+" "$|" "$?"
one two three four keep order item.src tem.src item.a tem.b dollar$$X: ;
