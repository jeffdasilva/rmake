held:
	+@printf 'held\n'; : >ready; \
	  until test -f release; do sleep 0.01; done; printf 'finished\n'
query:
	@$(file >queried,seen)printf 'must not run\n'
after:
	+@until test -f ready; do sleep 0.01; done; printf 'after\n'; : >release
