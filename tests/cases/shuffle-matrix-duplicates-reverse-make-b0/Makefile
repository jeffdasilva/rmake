$(file >seen,anchor)
$(info FLAG=[$(filter --shuffle=%,$(MAKEFLAGS))] GOALS=[$(MAKECMDGOALS)])
.PHONY: all a b c
all: a a b c a c
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
a: 
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
b: 
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
c: 
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
