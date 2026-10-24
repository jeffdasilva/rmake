V=old
ifeq ($(MAKE_RESTARTS),)
.PHONY: refresh
pre.mk: refresh
	@printf "V=new\n" > $@
endif
