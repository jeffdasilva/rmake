$(info before=[$(MAKEFLAGS)])
MAKEFLAGS = --jobs=-1 --jobs=1
$(info after=[$(MAKEFLAGS)])
.PHONY: all
all: ; @printf "%s\n" "recipe=[$(MAKEFLAGS)]" > seen
