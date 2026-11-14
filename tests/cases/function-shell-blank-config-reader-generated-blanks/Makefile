empty :=
sp := $(empty) $(empty)
tab := $(empty)	$(empty)
MARK := $(shell /bin/sh -c 'exit 9')
$(file >events,before)
SHELL = $(file >>events,SHELL[$(.SHELLSTATUS)])$(MISSING_SHELL)/bin/sh
.SHELLFLAGS = $(file >>events,FLAGS[$(.SHELLSTATUS)])$(MISSING_FLAGS)-c
IFS = $(file >>events,IFS[$(.SHELLSTATUS)])$(MISSING_IFS)$(sp)$(tab)
export WATCH = $(file >>events,EXPORT[$(.SHELLSTATUS)])x
$(info capture=[$(shell $(sp)$(tab))] status=[$(.SHELLSTATUS)])
$(file >>events,AFTER$(MISSING_AFTER))$(info order=[$(file <events)])
all:
