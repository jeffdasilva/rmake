$(file >seen,anchor)
.PHONY: all
all: dir/itemÿ.out
	@$(file >>seen,all)$(info all):
dir/%.out: ./%.in
	@$(file >>seen,$@)$(info T=[$@] FIRST=[$<] PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
itemÿ.in: ;
