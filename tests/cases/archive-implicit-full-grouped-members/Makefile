all: lib.a(one.x) lib.a(one.y)
lib.a(%.x) lib.a(%.y): %.src
	@printf "%s:%s\n" "$@" "$%"; cp result.a lib.a
