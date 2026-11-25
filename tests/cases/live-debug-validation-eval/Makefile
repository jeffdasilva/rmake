$(info START)
$(file >>seen,START)
define SETFLAGS
MAKEFLAGS := FIRST:=$$$$(file\ >>seen,FIRST)$$$$(info\ FIRST) --debug=z LAST:=$$$$(file\ >>seen,LAST)$$$$(info\ LAST)
endef
$(eval $(SETFLAGS))
include extra.mk
$(info END:$(origin FIRST):$(origin LAST))
$(file >>seen,END)
.PHONY: all
all: ; @printf "GOAL:%s:%s\n" "$(origin FIRST)" "$(origin LAST)"
