$(info START)
$(file >>seen,START)
MAKEFLAGS := FIRST:=$$(file\ >>seen,FIRST)$$(info\ FIRST$$(IGNORED_WARNING)) --warn-undefined-variables --debug=z LAST:=$$(file\ >>seen,LAST)$$(info\ LAST$$(REPORTED_WARNING))
$(info END:$(origin FIRST):$(origin LAST))
$(file >>seen,END)
.PHONY: all
all: ; @printf "GOAL:%s:%s\n" "$(origin FIRST)" "$(origin LAST)"
