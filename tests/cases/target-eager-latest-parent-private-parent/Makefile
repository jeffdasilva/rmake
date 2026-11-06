V := global
all: left right caller
left: V := first
right: private V := second
left right: dest
dest:;@$(info selected=[$(V)|$@]):
caller:;@$(eval dest: RESULT := $$(info retained=[$$(V)|$$@])):
