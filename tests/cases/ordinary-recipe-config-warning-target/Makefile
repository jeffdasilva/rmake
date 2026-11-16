empty :=
space := $(empty) $(empty)
STATUS := $(shell /bin/sh -c 'exit 9')
$(file >events,start)
export WATCH = $(file >>events,export[$(.SHELLSTATUS)])yes
all: SHELL = $(file >>events,shell[$(.SHELLSTATUS)])$(MISSING_SHELL)/bin/sh
all: .SHELLFLAGS = $(file >>events,flags[$(.SHELLSTATUS)])$(MISSING_FLAGS)-c
all: IFS = $(file >>events,ifs[$(.SHELLSTATUS)])$(MISSING_IFS)$(space)
all:
	@$(file >>events,body-1)printf "%s\n" first$(MISSING_BODY)
	@$(file >>events,body-2)printf "%s\n" second$(MISSING_AFTER)
