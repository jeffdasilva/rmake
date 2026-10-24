all: lib.a(one.o)
lib.a(one.o):
	@cp next.a lib.a; touch -d @1600000002 one.o; printf "rebuilt\n"
