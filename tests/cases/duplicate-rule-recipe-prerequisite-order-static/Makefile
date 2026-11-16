all: item.o
item.o: old | tail
item.o item.o: %.o: %.c first | order
	@printf '%s\n' '[$@][$*][$<][$^][$+][$|]'
item.c first old order tail: ;
