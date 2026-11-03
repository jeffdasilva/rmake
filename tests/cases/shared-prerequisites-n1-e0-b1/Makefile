$(file >seen,anchor)
.PHONY: all left right hidden n0ÿ
all:left right
	@$(file >>seen,all)$(info all PLUS=[$+] FIRST=[$<]):
left: n0ÿ n0ÿ
	@$(file >>seen,left)$(info left PLUS=[$+] NORMAL=[$^] FIRST=[$<] ORDER=[$|]):
right: n0ÿ n0ÿ
	@$(file >>seen,right)$(info right PLUS=[$+] NORMAL=[$^] FIRST=[$<] ORDER=[$|]):
n0ÿ:;@$(file >>seen,n0ÿ):
hidden:;@$(file >>seen,hidden):
