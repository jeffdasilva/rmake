include config.mk
$(info phase=new:$(notdir $(CURDIR)):$(MAKE_RESTARTS):$(MARKER):$(wildcard new*.in))
all: new.out
	@$(file >where,$(abspath .))printf '%s\n' NEW:$(MARKER)
%.out: %.in
	@cp $< $@
