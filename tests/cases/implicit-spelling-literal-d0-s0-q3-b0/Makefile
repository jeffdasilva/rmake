$(file >seen,anchor)
$(file >\\\literal,input)
.PHONY: all
all: item.out
	@$(file >>seen,all)$(info all):
%.out: \\\literal
	@$(file >>seen,$@)$(info T=[$@] FIRST=[$<] PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
