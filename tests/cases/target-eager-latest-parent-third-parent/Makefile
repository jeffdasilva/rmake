V := global
all: left middle right caller
left: V := first
middle: V := second
right: V := third
left middle right: dest
dest:;@$(info selected=[$(V)|$@]):
caller:;@$(eval dest: RESULT := $$(info retained=[$$(V)|$$@])):
