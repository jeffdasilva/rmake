$(info read=[$(filter -j% -l% -O%,$(MAKEFLAGS))])
GNUMAKEFLAGS = -S
.PHONY: all
all:;@printf 'flags=[%s]\n' '$(filter -j% -l% -O%,$(MAKEFLAGS))'
