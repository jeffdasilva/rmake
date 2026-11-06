.PHONY: all
all: unit.bundle
%.bundle: %.part %.index
	@cat $^ >$@
%.part %.index:
	@$(file >$*.part,part)$(file >$*.index,index)
