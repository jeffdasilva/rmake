seed_files := $(shell mkdir -p src && touch -d @1600000005 all && touch -d @1600000015 seed && touch -d @1600000010 item.o && ar crU src/libpack.a item.o && rm item.o && touch -d @1600000010 src/libpack.a)
VPATH := src
all: libpack.a(item.o)
	@printf 'parent=[%s|%s]\n' '$<' '$?'
libpack.a(item.o): seed
	@printf 'child=[%s|%s]\n' '$@' '$?'
