HOME = $(if $(wildcard home-first),./home-dir,$(file >home-first,first)~)
MAKEFLAGS += --include-dir=- --include-dir=~/inc
$(file >>seen,SEARCH|$(.INCLUDE_DIRS)|$(value MAKEFLAGS))
include part.mk
all: ; @$(file >>seen,RUN|$(MAKEFILE_LIST)|$(VALUE))cat seen
