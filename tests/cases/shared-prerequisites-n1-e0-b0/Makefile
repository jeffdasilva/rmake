$(file >seen,anchor)
.PHONY: all left right hidden n0
all:left right
	@$(file >>seen,all)$(info all PLUS=[$+] FIRST=[$<]):
left: n0 n0
	@$(file >>seen,left)$(info left PLUS=[$+] NORMAL=[$^] FIRST=[$<] ORDER=[$|]):
right: n0 n0
	@$(file >>seen,right)$(info right PLUS=[$+] NORMAL=[$^] FIRST=[$<] ORDER=[$|]):
n0:;@$(file >>seen,n0):
hidden:;@$(file >>seen,hidden):
