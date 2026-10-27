.SHELLFLAGS = $(if $(filter target,$@),$(if $(wildcard stage2),$(error $(file >failed)stopped),$(if $(wildcard stage1),$(file >stage2),$(file >stage1))))-c
all: peer target
peer:
	@python3 peer.py
	@touch peer.tail
target:
	-@missing_m5_completion
	-@:
	@touch forbidden
