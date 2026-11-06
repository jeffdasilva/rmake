P := global
d%: P := original-pattern
all: dest caller
dest:;@$(info selected=[$(P)|$@]):
caller: P := current-caller
caller:;@$(eval d%: P := late-pattern)$(eval dest: RESULT := $$(info destination=[$$(P)|$$(origin P)|$$@])):
