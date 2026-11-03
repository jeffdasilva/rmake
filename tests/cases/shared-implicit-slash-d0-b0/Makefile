$(file >seen,anchor)
.PHONY: all
all: item.out
	@$(file >>seen,all)$(info all):
%.out: ./%.in
	@$(file >>seen,$@)$(info T=[$@] FIRST=[$<] PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
item.in: ;
