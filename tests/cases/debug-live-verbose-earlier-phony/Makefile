.PHONY: prior
all: prior out final
	@printf 'parent\n'
prior:
	@printf 'prior\n'
out: seed
	$(eval MAKEFLAGS += --debug=v)cp seed out
final: prior out seed
	cp out final
