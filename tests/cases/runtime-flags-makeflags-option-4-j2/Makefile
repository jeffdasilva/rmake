$(info read=[$(filter -j% -l% -O%,$(MAKEFLAGS))])
MAKEFLAGS = -l
.PHONY: all
all:;@printf 'flags=[%s]\n' '$(filter -j% -l% -O%,$(MAKEFLAGS))'
