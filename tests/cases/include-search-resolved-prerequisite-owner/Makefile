HOME = $(if $(wildcard home-first),././home-dir,$(file >home-first,first)~)
holder: home-dir/inc/part.mk
MAKEFLAGS += --include-dir=- --include-dir=~/inc --include-dir=./home-dir/inc
$(file >>seen,SEARCH|$(.INCLUDE_DIRS)|$(value MAKEFLAGS))
include part.mk part.mk
all: ; @$(file >>seen,RUN|$(MAKEFILE_LIST)|$(VALUE))cat seen
