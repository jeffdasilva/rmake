.PHONY: all
all:
	+@MAKE_TMPDIR='missing-make' TMPDIR='missing-tmp' $(MAKE) --no-print-directory -f child.mk -j2 --jobserver-style=fifo 'EXPECT_DIR=/tmp' STYLE=fifo
	@if test -f owned-fifo-path; then test ! -e "$$(cat owned-fifo-path)" && rm owned-fifo-path; fi
