R0 = $(eval COUNT+=x)[$(R1)]
R1 = $(eval COUNT+=x)[$(R2)]
R2 = $(eval COUNT+=x)[$(R3)]
R3:=end
WRAP=[$(1)]
COUNT:=
.PHONY: all
all:;@$(info VALUE=[$(R0)])$(info COUNT=[$(words $(COUNT))]):
