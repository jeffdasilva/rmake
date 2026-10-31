R0 = $(eval PICK:=R1)[$($(PICK))]$(PICK)
R1 = $(eval PICK:=R2)[$($(PICK))]$(PICK)
R2 = $(eval PICK:=R3)[$($(PICK))]$(PICK)
R3:=end
PICK:=before
$(info VALUE=[$(R0)])$(info PICK=[$(PICK)])
.PHONY: all
all:;
