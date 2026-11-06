seed_time := $(shell touch -d @1600000000 unit.index)
.PHONY: all
all: unit.note
.INTERMEDIATE: unit.index
%.note %.index:
	@printf 'note\n' >$*.note
	@printf 'index\n' >$*.index
