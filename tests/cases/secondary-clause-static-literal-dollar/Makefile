item.o: %.o: dollar$$X
.SECONDEXPANSION:
item.o: i%.o: $$(info B @=[$$@] *=[$$*] <=[$$<] ^=[$$^] +=[$$+] |=[$$|] ?=[$$?])one
	@printf "@=[%s] *=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] ?=[%s]\n" "$@" "$*" "$<" "$^" "$+" "$|" "$?"
one two three four keep order item.src tem.src item.a tem.b dollar$$X: ;
