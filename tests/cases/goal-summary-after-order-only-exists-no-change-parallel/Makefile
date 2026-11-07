ifeq ($(MAKE_RESTARTS),)
$(shell mkdir -p pool; touch -d @1000000100 config.mk; touch -d @1000000300 product; touch -d @1000000200 middle)
endif
include config.mk
.PHONY: force
config.mk: force ; @printf "read-side [%s]\n" "$@"
product: | config.mk
product: ; @printf "product\n"
