MAKEFLAGS += --jobserver-auth=opaque
.PHONY: all
all:
	$(info advertised=[$(filter --jobserver-auth=%,$(MAKEFLAGS))])+@python3 wire.py
