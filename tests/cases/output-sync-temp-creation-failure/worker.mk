ifdef READER
$(info reader)
endif

.PHONY: all
all: ; @printf 'executing:%s\n' "$$MAKEFLAGS"; printf 'payload\n' > witness
