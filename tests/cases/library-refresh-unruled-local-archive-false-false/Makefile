ifeq ($(MAKE_RESTARTS),)
$(shell mkdir -p pool)
endif
all: libwidget.a
.LIBPATTERNS := lib%.a
all: -lwidget
	@printf "inputs=[%s]\n" "$^"
libwidget.a:
	@printf "building [%s]\n" "$@"
	@touch "$@"
