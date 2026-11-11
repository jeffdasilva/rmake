all: held release
	@printf 'parent\n' >forbidden-parent
held:
	+@printf 'held\n' >ready; until test -f release; do sleep 0.01; done
	@printf 'forbidden\n' >skipped
release:
	+@until test -f ready; do sleep 0.01; done; : >release
