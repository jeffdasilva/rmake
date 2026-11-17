all: earlier out final
	@printf 'parent\n'
earlier: ;
out: seed
	$(eval MAKEFLAGS += --debug=v)cp seed out
final: independent out
	cp out final
independent: seed
	cp seed independent
