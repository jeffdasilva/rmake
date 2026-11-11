ifeq ($(FROM_FILE),yes)
  MAKEFLAGS += --jobserver-style=fifo
endif

.PHONY: all
all: ; +@python3 ../observer.py verify-pipe
