.PHONY: all
all: unit.bundle
.NOTINTERMEDIATE: %.index
%.bundle: %.part %.index
	@cat $^ >$@
%.part %.index:
	@printf 'part\n' >$*.part
	@printf 'index\n' >$*.index
