all: held release
	+@printf 'parent\n'
held:
	+@printf 'held\n'; : >ready; \
	  until test -f release; do sleep 0.01; done; printf 'finished\n'
release:
	+@until test -f ready; do sleep 0.01; done; printf 'release\n'; : >release
