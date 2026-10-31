R0 = $(file >>seen,E)[$(R1)]
R1 = $(file >>seen,E)[$(R2)]
R2 = $(file >>seen,E)[$(R3)]
R3:=end
WRAP=[$(1)]
COUNT:=
.PHONY: all
all:;@$(info VALUE=[$(R0)])$(info EFFECT=[$(words $(file <seen))]):
