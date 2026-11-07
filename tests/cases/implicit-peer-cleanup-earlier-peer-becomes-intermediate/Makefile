.PHONY: all
all: unit.note unit.bundle
%.bundle: %.index
	@cat $^ >$@
%.note %.index:
	@printf 'note\n' >$*.note
	@printf 'index\n' >$*.index
