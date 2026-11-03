$(file >seen,anchor)
.PHONY: all
all: dir/itemÿ.out
	@$(file >>seen,all)$(info all):
%.out: \%.in
	@$(file >>seen,$@)$(info T=[$@] FIRST=[$<] PLUS=[$+] NORMAL=[$^] ORDER=[$|]):
\%.in: ;
