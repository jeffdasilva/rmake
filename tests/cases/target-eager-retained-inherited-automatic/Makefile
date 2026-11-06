V := global
all: left caller
left: V := first-parent
left: dest
dest: dep
dest:;@$(info selected=[$(V)|$@|$<|$^]):
dep:;
caller: V := current-caller
caller:;@$(eval dest: RESULT := $$(info destination=[$$(V)|$$@|$$<|$$^]))$(info caller=[$(V)|$@]):
