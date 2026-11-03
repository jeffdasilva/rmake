$(file >seen,anchor)
.PHONY: all
all: itemÿ.out
	@$(file >>seen,all)$(info all):
%.out: literalÿ
	@$(file >>seen,$@)$(info T=[$@] FIRST=[$<] PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
literalÿ: ;
