V := original-global
all: left caller
left: W := original-parent
left: dest
dest:;@:
caller:;@$(eval V := live-global)$(eval left: W := live-parent)$(eval dest: LOCAL := own-destination)$(eval dest: RESULT := $$(info destination=[$$(V)|$$(W)|$$(LOCAL)|$$@])):
