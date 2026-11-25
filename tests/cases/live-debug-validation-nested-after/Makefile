$(info START)
$(file >>seen,START)
MAKEFLAGS := --debug=z FIRST:=$$(eval\ MAKEFLAGS\ :=\ -s)$$(file\ >>seen,FIRST)$$(info\ FIRST) LAST:=$$(file\ >>seen,LAST)$$(info\ LAST)
$(info END:$(origin FIRST):$(origin LAST))
$(file >>seen,END)
.PHONY: all
all: ; @printf "GOAL:%s:%s\n" "$(origin FIRST)" "$(origin LAST)"
