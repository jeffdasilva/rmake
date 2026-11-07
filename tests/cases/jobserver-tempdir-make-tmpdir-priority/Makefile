.PHONY: all
all:
	+@MAKE_TMPDIR='$(CURDIR)/preferred' TMPDIR='$(CURDIR)/fallback' $(MAKE) --no-print-directory -f child.mk -j2 --jobserver-style=fifo 'EXPECT_DIR=$(CURDIR)/preferred' STYLE=fifo
	@if test -f owned-fifo-path; then test ! -e "$$(cat owned-fifo-path)" && rm owned-fifo-path; fi
