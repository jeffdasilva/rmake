$(info BEFORE=[$(words $(.INCLUDE_DIRS))] FLAGS=[$(words $(filter-out -I-,$(filter -I%,$(MAKEFLAGS))))])
GNUMAKEFLAGS = -Iinc/.
.PHONY: all
all:;@$(info PASSIVE=[$(words $(.INCLUDE_DIRS))])$(eval MAKEFLAGS += -s)$(info AFTER=[$(words $(.INCLUDE_DIRS))] FLAGS=[$(words $(filter-out -I-,$(filter -I%,$(MAKEFLAGS))))]):
