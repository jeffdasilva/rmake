ifeq ($(MAKE_RESTARTS),)
$(shell mkdir -p pool)
endif
.SECONDEXPANSION:
.LIBPATTERNS := lib%.a
all: -lwidget $$@.marker
	@printf "inputs=[%s]\n" "$^"
all.marker: ; @:
-lwidget: libwidget.a ; @:
libwidget.a:
	@printf "building [%s]\n" "$@"
	@touch "$@"
