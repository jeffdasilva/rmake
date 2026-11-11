.PHONY: stale
target: stale
	+@printf 'held\n'; : >ready; until test -f release; do sleep 0.01; done
	@printf 'must not run\n'
release:
	+@until test -f ready; do sleep 0.01; done; printf 'release\n'; : >release
