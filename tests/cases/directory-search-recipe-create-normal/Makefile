all: early.out .WAIT seed .WAIT later.out
%.out: d/new.in
	@echo found=$@:$<
seed: ; @cp input d/new.in
.DEFAULT: ; @echo fallback=$@
