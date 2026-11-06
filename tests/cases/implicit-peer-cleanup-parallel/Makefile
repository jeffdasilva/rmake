.PHONY: all
all: unit.bundle
%.bundle: %.part %.index unit.note
	@cat $^ >$@
%.part %.index %.note:
	@printf 'part\n' >$*.part
	@printf 'index\n' >$*.index
	@printf 'note\n' >$*.note
