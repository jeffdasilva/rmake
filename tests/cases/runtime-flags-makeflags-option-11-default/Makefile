$(info read=[$(filter -j% -l% -O%,$(MAKEFLAGS))])
MAKEFLAGS = -n
.PHONY: all
all:;@printf 'flags=[%s]\n' '$(filter -j% -l% -O%,$(MAKEFLAGS))'
