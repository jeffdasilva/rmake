$(file >seen,anchor)
.PHONY: all left right hidden 
all:left right
	@$(file >>seen,all)$(info all PLUS=[$+] FIRST=[$<]):
left right: .EXTRA_PREREQS=hidden
left: 
	@$(file >>seen,left)$(info left PLUS=[$+] NORMAL=[$^] FIRST=[$<] ORDER=[$|]):
right: 
	@$(file >>seen,right)$(info right PLUS=[$+] NORMAL=[$^] FIRST=[$<] ORDER=[$|]):
hidden:;@$(file >>seen,hidden):
