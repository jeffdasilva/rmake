$(file >seen,anchor)
.PHONY: all a b c d x y z
all: a b c d
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
a:
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
b:
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
c:
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
d:
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
x:
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
y:
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
z:
	@$(file >>seen,$@)$(info BUILD $@ PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
