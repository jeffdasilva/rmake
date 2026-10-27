$(info read=[$(filter -j% -l% -O%,$(MAKEFLAGS))])
MAKEFLAGS = -l2.5
.PHONY: all
all:;@printf 'flags=[%s]\n' '$(filter -j% -l% -O%,$(MAKEFLAGS))'
