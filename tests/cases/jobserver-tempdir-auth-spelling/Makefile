.PHONY: all
all:
	@mkdir -p relative owner/relative owner/elsewhere
	+@MAKE_TMPDIR='./relative///' $(MAKE) -C owner -f ../exercise.mk -j2 --jobserver-style=fifo 'FIXTURE=$(CURDIR)' 'EXPECT=./relative////' CHECK_CHDIR=yes
	+@MAKE_TMPDIR='$(CURDIR)/relative/' $(MAKE) -f exercise.mk -j2 --jobserver-style=fifo 'FIXTURE=$(CURDIR)' 'EXPECT=$(CURDIR)/relative//'
	@rmdir owner/elsewhere owner/relative owner relative
