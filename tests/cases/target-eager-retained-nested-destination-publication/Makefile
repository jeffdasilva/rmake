V := global
all: left caller
left: V := first-parent
left: dest
dest:;@:
caller:;@$(eval dest: OUTER := $$(eval dest: INNER := $$$$(V)|$$$$@)$$(info inner=[$$(INNER)]))$(eval dest: LATER := $$(info later=[$$(INNER)|$$@]))$(info caller=[$(INNER)|$@]):
