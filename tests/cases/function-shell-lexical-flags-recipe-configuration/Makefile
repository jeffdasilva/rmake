empty :=
space := $(empty) $(empty)
FIRST := $(shell /bin/sh -c 'exit 9')
$(file >events,read)
export WATCH = $(file >>events,export[$(.SHELLSTATUS)])yes
SHARED := old
SHELL = $(file >>events,shell[$(.SHELLSTATUS)])/bin/sh
.SHELLFLAGS = $(file >>events,flags[$(.SHELLSTATUS)])$(let SHELL,/bin/sh,$(let .SHELLFLAGS,-c,$(let IFS,,$(shell /bin/sh -c "exit 6"))))-c
IFS = $(file >>events,ifs[$(.SHELLSTATUS)])$(space)
all: target
target:
	echo "first $(SHARED)$$WATCH"; :
	@echo "second $$WATCH"
