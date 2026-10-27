$(info read=[$(filter -j% -l% -O%,$(MAKEFLAGS))])
GNUMAKEFLAGS = -j2
.PHONY: all
all:;@printf 'flags=[%s]\n' '$(filter -j% -l% -O%,$(MAKEFLAGS))'
