.PHONY: all
all: unit.bundle
%.bundle: %.part %.index unit.note
	@cat $^ >$@
%.note %.part %.index:
	@printf 'note\n' >$*.note
	@printf 'part\n' >$*.part
	@printf 'index\n' >$*.index
