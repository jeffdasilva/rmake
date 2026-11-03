$(file >seen,anchor)
$(file >\item-%.in,input)
.PHONY: all
all: dir/item.out
	@$(file >>seen,all)$(info all):
dir/%.out: \%-%.in
	@$(file >>seen,$@)$(info T=[$@] FIRST=[$<] PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
