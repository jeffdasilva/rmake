.PHONY: all
all: unit.bundle
.PRECIOUS: %.index
%.bundle: %.part %.index
	@cat $^ >$@
%.part %.index:
	@printf 'part\n' >$*.part
	@printf 'index\n' >$*.index
