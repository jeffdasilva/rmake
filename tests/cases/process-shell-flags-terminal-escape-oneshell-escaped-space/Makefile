empty :=
space := $(empty) $(empty)
backslash := \$(empty)
$(file >flags,head$(backslash)$(space))
SHELL := ./capture
.SHELLFLAGS := $(file <flags)
.ONESHELL:
all: ; @dummy extra
