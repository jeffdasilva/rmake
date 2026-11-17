all: before out final
	@printf 'parent\n'
before: seed
	$(eval MAKEFLAGS += --debug=n)cp seed before
out: seed
	$(eval MAKEFLAGS += --debug=v)cp seed out
final: before out
	cp out final
