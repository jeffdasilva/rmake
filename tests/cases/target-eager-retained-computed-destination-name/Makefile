KEY := global
all: left caller
left: KEY := inherited
left: dest
dest:;@:
caller: KEY := current-caller
caller:;@$(eval dest: READ_$$(KEY) := $$@)$(eval dest: RESULT := $$(info names=[$$(READ_inherited)|$$(READ_global)|$$(READ_current-caller)])):
