$(file >seen,anchor)
$(file >item-%.in,input)
.PHONY: all
all: item.out
	@$(file >>seen,all)$(info all):
%.out: %-%.in
	@$(file >>seen,$@)$(info T=[$@] FIRST=[$<] PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
