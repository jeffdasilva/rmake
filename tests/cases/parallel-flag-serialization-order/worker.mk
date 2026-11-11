.PHONY: owner peer
owner:
	+@python3 observer.py show
	+@$(MAKE) -f worker.mk peer
peer: ; @python3 observer.py show
