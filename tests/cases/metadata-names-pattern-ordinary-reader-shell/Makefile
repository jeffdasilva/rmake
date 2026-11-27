export .VARIABLES
.VARIABLES := retained
FRESH = yes
%.no: .VARIABLES := pattern
$(info refresh=[$(filter FRESH,$(shell printenv .VARIABLES))])
all: ; @:
