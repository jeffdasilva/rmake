export R0
R0 = [$(R1)]
R1 = [$(R2)]
R2 = [$(R3)]
R3 = $(shell printf '%s' "$$R0")
$(info VALUE=[$(R0)])$(info STATUS=[$(.SHELLSTATUS)])
.PHONY: all
all:;
