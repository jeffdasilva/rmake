all: out final
	@printf 'parent\n'
out:: seed
	$(eval MAKEFLAGS += --debug=v)cp seed out
out:: seed
	@printf 'second\n'
final: out
	cp out final
