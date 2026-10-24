.SECONDEXPANSION:
EXT=.src
item.o: %.o: lib.a(%$$(EXT))
item.o: i%.o: lib.a(%$$(EXT))
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+]"
lib.a(item.src tem.src): ;
