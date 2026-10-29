$(info BEFORE=[$(words $(file <seen))])
GNUMAKEFLAGS = -s
.PHONY: all
all:;@$(info PASSIVE=[$(words $(file <seen))])$(eval MAKEFLAGS += -s)$(info AFTER=[$(words $(file <seen))]):
