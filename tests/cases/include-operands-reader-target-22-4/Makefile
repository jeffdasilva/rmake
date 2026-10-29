$(info BEFORE=[$(words $(.INCLUDE_DIRS))] FLAGS=[$(words $(filter-out -I-,$(filter -I%,$(MAKEFLAGS))))])
MAKEFLAGS += -Iinc/.
$(info AFTER=[$(words $(.INCLUDE_DIRS))] FLAGS=[$(words $(filter-out -I-,$(filter -I%,$(MAKEFLAGS))))])
.PHONY: all
all:;@:
