$(file >seen,anchor)
$(info FLAG=[$(filter --shuffle=%,$(MAKEFLAGS))] GOALS=[$(MAKECMDGOALS)])
.PHONY: all aÿ bÿ cÿ dÿ
all: aÿ bÿ
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
aÿ: all cÿ
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
bÿ: dÿ
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
cÿ: 
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
dÿ: 
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
