$(shell test ! -f made.mk || touch -t 200001010000 made.mk)
include made.mk
$(info read flags=[$(.SHELLFLAGS)] restarts=[$(MAKE_RESTARTS)])
all: ; @printf done\n
ifndef READY
.PHONY: force
endif
made.mk: $(if $(READY),,force) ; @printf "READY=yes\n" > $@
