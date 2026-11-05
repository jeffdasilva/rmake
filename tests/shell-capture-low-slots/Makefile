load ./low_slots.so
$(close_slots $(CLOSED_SLOTS))
CAPTURE := $(shell $(CAPTURE_COMMAND))
STATUS := $(.SHELLSTATUS)
LOW := $(restore_slots )
$(file >>seen,status=[$(STATUS)]|low=[$(LOW)]|result=[$(CAPTURE)])
$(info status=[$(STATUS)]|low=[$(LOW)]|result=[$(CAPTURE)])
all:;@:
