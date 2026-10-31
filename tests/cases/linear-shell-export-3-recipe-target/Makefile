export R0
R0 = [$(R1)]
R1 = [$(R2)]
R2 = [$(R3)]
R3 = $(shell printf '%s' "$$R0")
.PHONY: all
all:;@$(info VALUE=[$(R0)])$(info STATUS=[$(.SHELLSTATUS)]):
