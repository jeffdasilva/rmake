$(file >seen,anchor)
include step.mk
.PHONY: last
last:;+@$(eval unexport MAKEFLAGS READY STATE MARK)$(info META EXTRA=[$(EXTRA)] STATE=[$(STATE)] GNU=[$(GNUMAKEFLAGS)] GOALS=[$(MAKECMDGOALS)] FILES=[$(MAKEFILE_LIST)])$(file >>seen,DONE)$(info SEEN=[$(file <seen)])python3 probe.py '$(filter -j%,$(MFLAGS))' '$(filter --jobserver-auth=%,$(MFLAGS))'
