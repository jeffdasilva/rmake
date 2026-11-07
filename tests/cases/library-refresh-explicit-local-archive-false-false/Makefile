ifeq ($(MAKE_RESTARTS),)
$(shell mkdir -p pool)
endif
.LIBPATTERNS := lib%.a
all: -lwidget
	@printf "inputs=[%s]\n" "$^"
-lwidget: libwidget.a ; @:
libwidget.a:
	@printf "building [%s]\n" "$@"
	@touch "$@"
