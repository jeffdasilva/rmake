$(file >seen,anchor)
$(info FLAG=[$(filter --shuffle=%,$(MAKEFLAGS))] GOALS=[$(MAKECMDGOALS)])
.PHONY: all g1 g2 a b c
all: g1 g2
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
g1 g2 &: a b c
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
a: 
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
b: 
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
c: 
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
