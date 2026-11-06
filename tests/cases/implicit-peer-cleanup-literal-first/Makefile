.PHONY: all
all: unit.bundle
%.bundle: unit.note %.part %.index
	@cat $^ >$@
%.part %.index %.note:
	@printf 'part\n' >$*.part
	@printf 'index\n' >$*.index
	@printf 'note\n' >$*.note
