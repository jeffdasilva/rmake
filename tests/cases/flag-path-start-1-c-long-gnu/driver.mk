$(info files=[$(MAKEFILE_LIST)] flags=[$(MAKEFLAGS)] mflags=[$(MFLAGS)] cwd=[$(notdir $(CURDIR))] value=[$(VALUE)])
all: ; @printf '%s\n' ROOT
