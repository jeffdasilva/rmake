$(info START)
$(file >>seen,START)
MAKEFLAGS := FIRST:=$$(eval\ MAKEFLAGS\ :=\ -s)$$(file\ >>seen,FIRST)$$(info\ FIRST) --debug=z LAST:=$$(file\ >>seen,LAST)$$(info\ LAST)
$(info END:$(origin FIRST):$(origin LAST))
$(file >>seen,END)
.PHONY: all
all: ; @printf "GOAL:%s:%s\n" "$(origin FIRST)" "$(origin LAST)"
