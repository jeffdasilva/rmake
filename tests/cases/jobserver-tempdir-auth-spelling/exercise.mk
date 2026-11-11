.PHONY: owner peer unavailable
owner:
	@python3 '$(FIXTURE)/observer.py' owner '$(EXPECT)'
	+@$(MAKE) -f '$(FIXTURE)/exercise.mk' peer
	+@if test '$(CHECK_CHDIR)' = yes; then \
	  MAKE_TMPDIR='' $(MAKE) -C elsewhere -f '$(FIXTURE)/exercise.mk' unavailable 2>chdir.err && \
	  python3 '$(FIXTURE)/observer.py' warnings && rm chdir.err; \
	fi
	@rm owned-fifo-path

peer:
	@python3 '$(FIXTURE)/observer.py' peer

unavailable:
	@python3 '$(FIXTURE)/observer.py' unavailable
