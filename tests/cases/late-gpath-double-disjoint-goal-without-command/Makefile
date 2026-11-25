$(info initial-goals=[$(MAKECMDGOALS)])
VPATH = src
GPATH = src
colon := :
READ = $(eval src/parent$(colon) SEEN := $$(info read=[$$@|$$<|$$(.SHELLSTATUS)|$$(SOURCE_ONLY)|$$(DESTINATION_ONLY)]))
parent: SOURCE_ONLY := source
src/parent: DESTINATION_ONLY := destination
parent:: input
	@$(shell exit 7)$(info head=[$@|$<|$(.SHELLSTATUS)])$(file >src/parent)
parent:: other
	@$(shell exit 13)$(info later=[$@|$<|$(.SHELLSTATUS)])
after:
	@$(info late-goals=[$(MAKECMDGOALS)])$(READ):
