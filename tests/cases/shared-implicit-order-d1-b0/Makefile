$(file >seen,anchor)
.PHONY: all
all: dir/item.out
	@$(file >>seen,all)$(info all):
%.out: literal literal | order
	@$(file >>seen,$@)$(info T=[$@] FIRST=[$<] PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
literal: ;
order: ;
