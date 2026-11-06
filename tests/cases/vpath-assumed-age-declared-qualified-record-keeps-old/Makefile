seed_files := $(shell mkdir -p src next && touch -d @1600000005 all && touch -d @1600000015 seed && touch -d @1600000010 src/x)
VPATH := src next
all: x
	@printf 'parent=[%s|%s]\n' '$<' '$?'
x: seed
	@printf 'child=[%s|%s]\n' '$@' '$?'
src/x:
