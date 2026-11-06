define FRAGMENT
$$(info inner-target): %: $$(error tail)
	@$$(error attached)
endef
$(eval $(FRAGMENT))
all:;@printf "all\n"
