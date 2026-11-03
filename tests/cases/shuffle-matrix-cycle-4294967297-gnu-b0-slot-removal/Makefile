$(file >seen,anchor)
$(info FLAG=[$(filter --shuffle=%,$(MAKEFLAGS))] GOALS=[$(MAKECMDGOALS)])
.PHONY: all a b c d
all: a b
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
a: all c
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
b: d
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
c: 
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
d: 
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
