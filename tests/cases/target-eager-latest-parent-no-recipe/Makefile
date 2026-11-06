V := global
all: left right caller
left: V := first
right: V := second
left right: dest
.PHONY: dest
caller:;@$(eval dest: RESULT := $$(info retained=[$$(V)|$$@])):
