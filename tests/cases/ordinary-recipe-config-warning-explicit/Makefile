empty :=
space := $(empty) $(empty)
STATUS := $(shell /bin/sh -c 'exit 9')
$(file >events,start)
export WATCH = $(file >>events,export[$(.SHELLSTATUS)])yes
SHELL = $(file >>events,shell[$(.SHELLSTATUS)])$(warning CONFIG)$(MISSING_SHELL)/bin/sh
.SHELLFLAGS = $(file >>events,flags[$(.SHELLSTATUS)])$(MISSING_FLAGS)-c
IFS = $(file >>events,ifs[$(.SHELLSTATUS)])$(MISSING_IFS)$(space)
all:
	+@$(file >>events,body)printf "%s\n" forced$(MISSING_BODY)
