all: a
a: old
./a a a: first second | order
	@printf '%s\n' '[$@][$^][$+][$|]'
first second old order: ;
