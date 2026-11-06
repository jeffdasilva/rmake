V := global
all: left right caller
left: V := first
right: V := second
left right: dest
dest:;@$(info selected=[$(V)|$@]):
caller:;@$(eval right: V := revised)$(eval dest: RESULT := $$(info retained=[$$(V)|$$@])):
