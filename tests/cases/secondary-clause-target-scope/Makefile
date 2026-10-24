.SECONDEXPANSION:
item.o: TAG=target
item.o: %.o: $$(info A=$$(TAG)-$$*)one
item.o: i%.o: $$(info B=$$(TAG)-$$*)two
	@printf "@=[%s] *=[%s] <=[%s] ^=[%s] +=[%s] |=[%s] ?=[%s]\n" "$@" "$*" "$<" "$^" "$+" "$|" "$?"
one two three four keep order item.src tem.src item.a tem.b dollar$$X: ;
