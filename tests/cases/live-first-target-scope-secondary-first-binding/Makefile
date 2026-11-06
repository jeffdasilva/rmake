LATE = $(eval all: LIVE := ready)$(LIVE)
.SECONDEXPANSION:
all: $$(LATE);@$(info deps=[$^] live=[$(LIVE)]):
ready:;@$(file >>seen,ready):
