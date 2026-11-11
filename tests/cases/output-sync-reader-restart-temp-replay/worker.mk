ifeq ($(READER),warning)
$(warning READER-WARNING:$(MAKE_RESTARTS))
else ifeq ($(READER),shell)
reader := $(shell printf "READER-SHELL:%s\n" "$(MAKE_RESTARTS)" >&2)
else
$(info READER-OUT:$(MAKE_RESTARTS))
endif

ifeq ($(OPTIONAL),yes)
-include generated.mk
else
include generated.mk
endif

all:
	@printf "ORDINARY:%s:%s\n" "$(MAKE_RESTARTS)" "$(VALUE)"; printf "ORDINARY-ERR\n" >&2
	@printf "ORDINARY-NEXT\n"

generated.mk:
	@printf "REMAKE:%s\n" "$(MAKE_RESTARTS)"; printf "REMAKE-ERR\n" >&2; \
	if test "$(REPAIR)" = yes; then mkdir relative; fi; \
	printf "VALUE := loaded\n" > generated.mk
	@printf "REMAKE-NEXT\n"
