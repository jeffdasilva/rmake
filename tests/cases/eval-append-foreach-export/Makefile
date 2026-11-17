V = global
all: ; @$(foreach V,inner,$(eval export V += suffix)$(info inside=[$(V)]))printf 'child=[%s]\n' "$$V"
later: ; @$(info outside=[$(V)/$(origin V)/$(flavor V)]):
