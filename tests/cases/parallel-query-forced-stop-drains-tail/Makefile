.DELETE_ON_ERROR:
held:
	+@printf 'held\n'; : >ready; \
	  until test -s stop.pid; do sleep 0.01; done; IFS= read -r pid <stop.pid; \
	  until test ! -d "/proc/$$pid"; do sleep 0.01; done; \
	  rm stop.pid; printf 'finished\n'
	+@printf 'tail\n'
stop:
	+@until test -f ready; do sleep 0.01; done; printf 'stop\n'; \
	  printf 'changed\n' >stop; printf '%s\n' "$$$$" >stop.pid; exit 1
after:
	+@printf 'must not run\n'
