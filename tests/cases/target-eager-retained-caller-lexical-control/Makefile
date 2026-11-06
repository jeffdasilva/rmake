V := global
all: left caller
left: V := first-parent
left: dest
dest:;@:
caller:;@$(foreach V,lexical,$(eval dest: RESULT := $$(info destination=[$$(V)|$$@]))$(info lexical=[$(V)|$@])):
