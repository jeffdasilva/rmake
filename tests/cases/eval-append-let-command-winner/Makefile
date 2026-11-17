APPEND = $(eval V += $$(file >>seen,RHS=[$$(V)])ignored)
all: ; @$(let V,local,$(APPEND))$(info outside=[$(V)/$(origin V)/$(flavor V)])cat seen
