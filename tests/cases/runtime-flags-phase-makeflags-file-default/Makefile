$(info before)
MAKEFLAGS = $(warning flags)-j2
$(info after=[$(filter -j%,$(MAKEFLAGS))])
.PHONY: all
all:;@printf 'flags=[%s]\n' '$(filter -j%,$(MAKEFLAGS))'
