V := global
d%: P := original
all: left right caller
left: V := first
right: V := second
left right: dest
dest:;@$(info selected=[$(V)|$(P)|$@]):
caller:;@$(eval d%: P := later)$(eval dest: RESULT := $$(info retained=[$$(V)|$$(P)|$$@])):
