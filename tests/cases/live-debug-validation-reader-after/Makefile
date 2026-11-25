$(info START)
$(file >>seen,START)
MAKEFLAGS := FIRST:=$$(file\ >>seen,FIRST)$$(info\ FIRST) LAST:=$$(file\ >>seen,LAST)$$(info\ LAST) --debug=z
include extra.mk
$(info END:$(origin FIRST):$(origin LAST))
$(file >>seen,END)
.PHONY: all
all: ; @printf "GOAL:%s:%s\n" "$(origin FIRST)" "$(origin LAST)"
