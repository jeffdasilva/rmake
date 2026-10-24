.SECONDEXPANSION:
all: one.out
%.out: $$(info primary=$$*)%.src
	@cp -p $< $@
%.out: $$(info fallback=$$*)%.fallback
	@cp -p $< $@
