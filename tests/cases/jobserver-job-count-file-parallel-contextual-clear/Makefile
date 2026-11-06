$(info before=[$(MAKEFLAGS)])
MAKEFLAGS := --jobs=-1 -I~/include
$(info after=[$(subst $(HOME)/,<home>/,$(MAKEFLAGS))])
.PHONY: all
all: ; @printf "%s\n" "recipe=[$(MAKEFLAGS)]" > seen
