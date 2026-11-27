export .VARIABLES
.VARIABLES := retained
FRESH = yes
%.no: override .VARIABLES += pattern
$(info shell=[$(shell printenv .VARIABLES)])
all: ; @:
