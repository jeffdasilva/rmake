ifeq ($(MAKE_RESTARTS),)
$(shell mkdir -p pool)
endif
all: -lwidget
	@printf "inputs=[%s]\n" "$^"
.LIBPATTERNS := lib%.a
-l%: lib%.a ; @:
libwidget.a: ; @touch $@
