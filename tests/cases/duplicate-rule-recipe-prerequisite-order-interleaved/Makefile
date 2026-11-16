all: a b
a: earlier-a
b: earlier-b sibling | tail
a b a b: first second | order
	@printf '%s\n' '[$@][$^][$+][$|]'
first second earlier-a earlier-b sibling order tail: ;
