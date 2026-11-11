.PHONY: owner peer
owner:
	@python3 '$(FIXTURE)/observer.py' show
	+@$(MAKE) -f worker.mk peer
	+@MAKE_TMPDIR='' $(MAKE) -C elsewhere -f ../worker.mk peer
peer: ; @python3 '$(FIXTURE)/observer.py' show
