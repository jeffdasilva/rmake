$(info reader)
ifeq ($(READER_OFF),yes)
MAKEFLAGS += -Onone
endif

.PHONY: all
all: ; @printf 'recipe\n'; printf 'payload\n' > witness
