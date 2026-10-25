.SECONDARY:
.SECONDEXPANSION:
all: out
out: x.mid
	@echo out changed=$?; cp -p $< $@
x.mid:: $$(info first=$$@)first
	@echo first; cp -p $< $@
x.mid:: $$(info second=$$@)second
	@echo second; cp -p $< $@
