all: dest caller
dest:;@$(shell /bin/sh -c 'exit 7'):
caller:;@$(shell /bin/sh -c 'exit 11')$(eval dest: OUTER := $$(eval caller: INNER := $$$$(shell /bin/sh -c 'exit 19'))$$(info retained=[$$(.SHELLSTATUS)|$$@]))$(info caller=[$(.SHELLSTATUS)|$@]):
