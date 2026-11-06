empty :=
space := $(empty) $(empty)
backslash := \$(empty)
define linefeed


endef
$(file >command.txt,head ''$(backslash)$(linefeed)$(space)$(space)$(space)$(space))
SHELL := ./capture
.SHELLFLAGS := $(file <command.txt)
all: ; @dummy
