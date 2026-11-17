.SECONDEXPANSION:
all: out final
	@printf 'parent\n'
out: seed $$(eval MAKEFLAGS += --debug=v)
	cp seed out
final: out
	cp out final
