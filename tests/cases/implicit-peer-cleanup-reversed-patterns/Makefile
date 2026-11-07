.PHONY: all
all: unit.bundle
%.bundle: %.part %.index
	@cat $^ >$@
%.index %.part:
	@printf 'index\n' >$*.index
	@printf 'part\n' >$*.part
