$(info START)
$(file >>seen,START)
MAKEFLAGS := FIRST:=$$(file\ >>seen,FIRST)$$(info\ FIRST) --jobs=0 --debug=z LAST:=$$(file\ >>seen,LAST)$$(info\ LAST)
$(info END:$(origin FIRST):$(origin LAST))
$(file >>seen,END)
.PHONY: all
all: ; @printf "GOAL:%s:%s\n" "$(origin FIRST)" "$(origin LAST)"
