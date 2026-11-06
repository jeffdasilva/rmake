MAKEFLAGS += --jobserver-auth=opaque
.PHONY: all waiting release
all: waiting release
waiting:
	@python3 witness.py
release:
	@touch released
