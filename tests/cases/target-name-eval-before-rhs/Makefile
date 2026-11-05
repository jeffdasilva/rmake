chosen: SOURCE := target
chosen: $(file >>seen,name=[$(SOURCE)])$(eval KEY := SLOT)$(KEY) := $(file >>seen,rhs=[$(KEY)])$(SOURCE)-value
$(info global-key=[$(KEY)] global-slot=[$(SLOT)])
.PHONY: all chosen
all: chosen
chosen:;$(info slot=[$(SLOT)] origin=$(origin SLOT))@:
