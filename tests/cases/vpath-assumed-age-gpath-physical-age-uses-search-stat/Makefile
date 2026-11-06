seed_files := $(shell mkdir -p src next && touch -d @1600000020 all && touch -d @1600000000 seed && touch -d @1600000010 real && ln -s ../real src/x && touch -h -d @1600000030 src/x)
VPATH := src next
GPATH := src next
all: x
	@printf 'parent=[%s|%s]\n' '$<' '$?'
x: seed
	@printf 'child=[%s|%s]\n' '$@' '$?'
