$(file >seen,anchor)
override GNUMAKEFLAGS = -j3 READY:=yes STATE:=$$(if\ $$(value\ GNUMAKEFLAGS),pending,cleared) MARK:=$$(file\ >>seen,GNU)
override MAKEFLAGS = $(if $(filter environment,$(origin READY)),$(file >>seen,FLAGS) -j1)
.PHONY: last
last:;+@$(eval unexport MAKEFLAGS READY STATE MARK)$(info META EXTRA=[$(EXTRA)] STATE=[$(STATE)] GNU=[$(GNUMAKEFLAGS)] GOALS=[$(MAKECMDGOALS)] FILES=[$(MAKEFILE_LIST)])$(file >>seen,DONE)$(info SEEN=[$(file <seen)])python3 probe.py '$(filter -j%,$(MFLAGS))' '$(filter --jobserver-auth=%,$(MFLAGS))'
