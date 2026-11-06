seed_files := $(shell mkdir -p src next && touch -d @1600000020 all && touch -d @1600000000 seed && touch -d @1600000010 actual && ln -s actual intermediate && touch -h -d @1600000040 intermediate && ln -s ../intermediate src/x && touch -h -d @1600000015 src/x)
VPATH := src next
GPATH := src
all: x
	@printf 'parent=[%s|%s]\n' '$<' '$?'
x: seed
	@printf 'child=[%s]\n' '$@'
