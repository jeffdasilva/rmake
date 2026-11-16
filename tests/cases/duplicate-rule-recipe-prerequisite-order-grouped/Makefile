all: a b
a: old | tail
	@printf '%s\n' 'prior'
a b a &: first second | order
	@printf '%s\n' '[$@][$<][$^][$+][$|]'
first second old order tail: ;
