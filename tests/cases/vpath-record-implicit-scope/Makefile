VPATH = src
all: item.out
%.out: %.in
	@echo implicit=$@:$<:$(VALUE)
src/item.in: VALUE=destination
src/item.in: ; @echo leaf=$@:$(VALUE)
