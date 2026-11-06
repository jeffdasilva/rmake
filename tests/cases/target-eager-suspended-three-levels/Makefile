all: first second caller
first:;@$(shell /bin/sh -c 'exit 7'):
second:;@$(shell /bin/sh -c 'exit 8'):
caller:;@$(shell /bin/sh -c 'exit 11')$(eval first: OUTER := $$(eval second: MID := $$$$(eval caller: INNER := $$$$$$$$(info outer=[$$$$$$$$(.SHELLSTATUS)|$$$$$$$$@]))$$$$(info middle=[$$$$(.SHELLSTATUS)|$$$$@]))$$(info first=[$$(.SHELLSTATUS)|$$@]))$(info caller=[$(.SHELLSTATUS)|$@]):
