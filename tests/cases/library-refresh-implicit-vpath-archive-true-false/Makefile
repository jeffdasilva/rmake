ifeq ($(MAKE_RESTARTS),)
$(shell mkdir -p pool)
endif
.SECONDEXPANSION:
.LIBPATTERNS := lib%.a
all: -lwidget $$@.marker
	@printf "inputs=[%s]\n" "$^"
all.marker: ; @:
VPATH := pool
-l%: pool/lib%.a ; @:
pool/libwidget.a:
	@printf "building [%s]\n" "$@"
	@touch "$@"
