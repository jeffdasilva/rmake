.DEFAULT: ;
.SECONDEXPANSION:
obj/./a.o: obj/%.o: ./%.in $$(strip $$(space)) | ./%.order
	@printf '%s\n' '[$*][$<][$^][$+][$|]'
