define BAD
ÿ
endef
CHECK = $(call intcmp,$(1),$(2))
all:
	@printf '%s\n' '$(call CHECK,$$(BAD),0)'
