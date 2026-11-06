all: caller
caller:;@$(shell /bin/sh -c 'exit 11')$(eval dest: OUTER := $$(eval caller: INNER := $$$$(info inside=[$$$$(.SHELLSTATUS)|$$$$@]))$$(info unvisited=[$$(.SHELLSTATUS)|$$@]))$(info caller=[$(.SHELLSTATUS)|$@]):
