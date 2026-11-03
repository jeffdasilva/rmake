$(file >seen,anchor)
$(file >itemÿ.in,input)
.PHONY: all
all: dir/itemÿ.out
	@$(file >>seen,all)$(info all):
dir/%.out: %.in
	@$(file >>seen,$@)$(info T=[$@] FIRST=[$<] PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
