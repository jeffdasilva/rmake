.DEFAULT: ;
obj/./a.o: older | tail
obj/./a.o obj/./a.o &: obj/%.o: ./%.in | ./%.order
	@printf '%s\n' '[$*][$<][$^][$+][$|]'
