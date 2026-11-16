all: a
a: old | tail
a a: first second first | order
	@printf '%s\n' '[$@][$<][$^][$+][$|]'
first second old order tail: ;
