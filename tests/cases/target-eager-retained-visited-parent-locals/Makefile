all: left caller
left: dest
left:;@$(shell /bin/sh -c 'exit 9')$(info parent=[$(.SHELLSTATUS)|$@]):
.PHONY: dest
caller:;@$(eval dest: RESULT := $$(info dest=[$$(.SHELLSTATUS)|$$@])):
