$(file >seen,anchor)
$(info FLAG=[$(filter --shuffle=%,$(MAKEFLAGS))] GOALS=[$(MAKECMDGOALS)])
.PHONY: all a b c x y z
all: a b c
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
a: x y
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
b: y z
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
c: 
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
x: 
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
y: 
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
z: 
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
