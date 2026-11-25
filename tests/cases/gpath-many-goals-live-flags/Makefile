filler = n00 n01 n02 n03 n04 n05 n06 n07 n08 n09 n10 n11 n12 n13 n14 n15 n16 n17 n18 n19 n20 n21 n22 n23 n24 n25 n26 n27 n28 n29 n30 n31
.PHONY: before $(filler) middle verify
prepare := $(shell touch -d @1600000000 src/foo.out; touch -d @1600000002 input)
VPATH = src
GPATH = src
before:
	@$(eval MAKEFLAGS += -I~)printf 'before:%s\n' '$(words $(MAKECMDGOALS))'
foo.out: input
	@printf 'update:%s\n' '$@'; cp $< $@
$(filler):
	@echo '$@'
middle:
	@echo middle
verify:
	@test ! -e foo.out && test "$$(cat src/foo.out)" = new && echo verify
