include settings.mk
$(info restart=[$(MAKE_RESTARTS)]:$(V))
all:;@:
ifeq ($(MAKE_RESTARTS),)
.PHONY: refresh
inc/settings.mk: refresh
	@printf "V=new\n" > $@
endif
