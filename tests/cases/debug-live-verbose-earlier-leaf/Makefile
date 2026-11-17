all: earlier out final
	@printf 'parent\n'
earlier: ;
out: seed
	$(eval MAKEFLAGS += --debug=v)cp seed out
final: earlier out seed
	cp out final
