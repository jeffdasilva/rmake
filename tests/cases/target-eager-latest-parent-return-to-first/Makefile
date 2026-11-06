V := global
all: left caller
left: V := first
left:: dest
left:: right dest
right: V := second
right: dest
right:;@$(eval dest: OWN := $$(shell /bin/sh -c 'exit 7'))$(eval dest: DURING := $$(info during=[$$(V)|$$@|$$(.SHELLSTATUS)])):
dest:;@$(info selected=[$(V)|$@]):
caller:;@$(eval dest: RESULT := $$(info retained=[$$(V)|$$@|$$(.SHELLSTATUS)])):
