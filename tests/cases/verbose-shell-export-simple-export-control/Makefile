export CYCLE := literal
OBS := $(shell printf "child=[%s]" "$$CYCLE"; printf "child\n" >>seen)
all:;@$(info result=[$(OBS)]):
