V := global
all: outer caller
outer: V := enclosing
outer: dest
dest: leaf
leaf: V := discarded
leaf: dest
.PHONY: leaf
dest:;@$(info selected=[$(V)|$@]):
caller:;@$(eval dest: RESULT := $$(info retained=[$$(V)|$$@])):
