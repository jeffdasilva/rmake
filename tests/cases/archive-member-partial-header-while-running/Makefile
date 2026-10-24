all: lib.a(one.o) helper
lib.a(one.o):
	@cp invalid.a lib.a; sleep .08; cp restored.a lib.a
helper:;@sleep .03
