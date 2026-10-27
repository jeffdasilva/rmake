.SHELLFLAGS = $(if $(wildcard trigger),$(error stopped),-c)
.INTERMEDIATE: mid
all: mid .WAIT target
mid:;@touch mid
target:
	@$(file >trigger)
	@touch forbidden
