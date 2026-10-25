VPATH = src
verify: foo.out
	@echo result=$<:$(file <$<)
foo.out: ORIGINAL=source
ifneq ($(RECORD),off)
src/foo.out: input
	@echo built=$@:$(ORIGINAL); cp -p $< $@
endif
