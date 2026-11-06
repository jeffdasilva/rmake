V := global
all: left right caller
left: V := first
right: V := second
left: dest
right: ./dest
dest:;@$(info selected=[$(V)|$@]):
caller:;@$(eval ./dest: RESULT := $$(info retained=[$$(V)|$$@])):
