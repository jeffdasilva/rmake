# Keep the older-prerequisite trace independent of checkout timestamps.
$(shell touch -t 200001010000.00 original && touch -t 200001010001.00 keep.mk)
include keep.mk
all: ; @:
keep.mk:: original ; @:
