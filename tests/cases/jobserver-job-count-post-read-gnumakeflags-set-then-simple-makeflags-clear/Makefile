$(info before=[$(MAKEFLAGS)])
GNUMAKEFLAGS = --jobs=1
MAKEFLAGS := --jobs=-1
$(info after=[$(MAKEFLAGS)])
.PHONY: all
all: ; @printf "%s\n" "recipe=[$(MAKEFLAGS)]" > seen
