$(file >seen,anchor)
$(file >itemÿ.in,input)
.PHONY: all
all: itemÿ.out
	@$(file >>seen,all)$(info all):
%.out: %.in
	@$(file >>seen,$@)$(info T=[$@] FIRST=[$<] PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
