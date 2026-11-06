V := global
all: left right caller
left: V := first
right: V := second
left right: dest
dest: dep
dest:;@$(info selected=[$(V)|$@|$<|$^]):
dep:;
caller:;@$(eval dest: RESULT := $$(info retained=[$$(V)|$$@|$$<|$$^])):
