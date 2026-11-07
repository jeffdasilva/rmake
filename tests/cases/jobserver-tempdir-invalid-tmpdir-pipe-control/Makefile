.PHONY: all
all:
	+@MAKE_TMPDIR='' TMPDIR='missing' $(MAKE) --no-print-directory -f child.mk -j2 --jobserver-style=pipe 'EXPECT_DIR=' STYLE=pipe
	@if test -f owned-fifo-path; then test ! -e "$$(cat owned-fifo-path)" && rm owned-fifo-path; fi
