$(info metadata=$(origin MAKEFILES):$(flavor MAKEFILES))
BEFORE:=$(shell if test "$${MAKEFILES+x}" = x; then printf set; else printf unset; fi)
MAKEFILES=pre.mk
AFTER:=$(shell printf "%s" "$$MAKEFILES")
$(info exported=$(BEFORE):$(AFTER))
all:;@:
