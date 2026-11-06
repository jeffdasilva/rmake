.PHONY: all
all: unit.bundle
%.bundle: %.part %.index
	@cat $^ >$@
%.part %.index:
	@printf 'part\n' >$*.part
	@printf 'index\n' >$*.index
