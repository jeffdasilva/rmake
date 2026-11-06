V := global
H := global-hidden
all: left caller
left: private H := hidden-parent
left: V := first-parent
left: dest
dest: private OWN := private-destination
dest:;@:
caller: OWN := current-caller
caller:;@$(eval dest: RESULT := $$(info destination=[$$(V)|$$(H)|$$(OWN)|$$@])):
