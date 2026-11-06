V := global
all: left right
left: V := first
right: V := second
left right: dest
dest:;@$(info selected=[$(V)|$@]):
right:;@$(shell /bin/sh -c 'exit 19')$(eval dest: RESULT := $$(info retained=[$$(V)|$$@|$$(.SHELLSTATUS)])):
