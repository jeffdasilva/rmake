all: dest caller
dest:;@$(shell /bin/sh -c 'exit 7')$(info selected=[$(.SHELLSTATUS)|$@]):
caller:;@$(shell /bin/sh -c 'exit 11')$(eval dest: FIRST := $$(info before=[$$(.SHELLSTATUS)|$$@])$$(shell /bin/sh -c 'exit 3'))$(eval dest: SECOND := $$(info after=[$$(.SHELLSTATUS)|$$@]))$(info caller=[$(.SHELLSTATUS)|$@]):
