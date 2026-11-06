.PHONY: all
all: unit.bundle
%.bundle: %.part unit.note %.index
	@cat $^ >$@
%.part %.index %.note:
	@printf 'part\n' >$*.part
	@printf 'index\n' >$*.index
	@printf 'note\n' >$*.note
