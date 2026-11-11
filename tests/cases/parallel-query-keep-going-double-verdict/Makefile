all::
	@printf 'must not run\n'
all::
	+@printf 'held\n'; : >ready; \
	  until test -s failure.pid; do sleep 0.01; done; IFS= read -r pid <failure.pid; \
	  until test ! -d "/proc/$$pid"; do sleep 0.01; done; \
	  rm failure.pid; printf 'finished\n'
failure:
	+@until test -f ready; do sleep 0.01; done; printf 'failure\n'; \
	  printf '%s\n' "$$$$" >failure.pid; exit 7
