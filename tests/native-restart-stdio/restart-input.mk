ifeq ($(LOAD_ON_READ),yes)
load ./probe.so
endif

probe.so: replacement.so
	@$(file >>seen,remake-module)cp replacement.so stage.so
	@mv stage.so probe.so

ifneq ($(MAKE_RESTARTS),)
include part.mk
part.mk:;@$(file >>seen,remake-include)printf 'PART := ready\n' >part.mk
endif

STATE = $(MAKE_RESTARTS)|$(.LOADED)|$(probe_image )|$(words $(MAKEFILE_LIST))|$(words $(wildcard $(filter $(TMPDIR)/%,$(MAKEFILE_LIST))))
$(file >>seen,read-$(STATE))
export PUBLIC = export-$(MAKE_RESTARTS)-$(PART)
.PHONY: all
all:
	@$(file >>seen,goal-$(STATE))printf '%s\n' "child=[$$PUBLIC]"
