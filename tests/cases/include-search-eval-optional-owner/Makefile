HOME = $(if $(wildcard home-first),./home-dir,$(file >home-first,first)~)
home-dir/inc/part.mk:
MAKEFLAGS += --include-dir=- --include-dir=~/inc
$(file >>seen,SEARCH|$(.INCLUDE_DIRS)|$(value MAKEFLAGS))
$(eval $(eval -include part.mk))
all: ; @$(file >>seen,RUN|$(MAKEFILE_LIST)|$(VALUE))cat seen
