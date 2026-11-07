.PHONY: all
all: unit.bundle
%.bundle: %.index %.part
	@cat $^ >$@
%.part %.index:
	@printf 'part\n' >$*.part
	@printf 'index\n' >$*.index
