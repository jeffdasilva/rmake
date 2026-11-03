$(file >seen,anchor)
$(file >\literal,input)
.PHONY: all
all: dir/item.out
	@$(file >>seen,all)$(info all):
dir/%.out: \literal
	@$(file >>seen,$@)$(info T=[$@] FIRST=[$<] PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
