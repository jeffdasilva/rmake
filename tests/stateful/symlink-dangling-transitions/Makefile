$(shell python3 setup.py)
.DEFAULT_GOAL := out
PHASE := $(file <phase)
TIME_0 = 1600000030
TIME_1 = 1600000030
TIME_2 = 1600000040
TIME_3 = 1600000060
TIME_4 = 1600000080
TIME_5 = 1600000100
TIME_6 = 1600000100
out: link
	@printf 'phase=%s\n' '$(PHASE)' >out
	@touch -d @$(TIME_$(PHASE)) out
	@printf 'built phase=%s newer=%s\n' '$(PHASE)' '$?'
.PHONY: verify
verify:;@cat out
