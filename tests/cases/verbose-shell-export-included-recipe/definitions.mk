# included provenance
export CYCLE = $(shell printf "child=[%s]" "$$CYCLE"; printf "child\n" >>seen)
