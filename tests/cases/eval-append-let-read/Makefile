V = global
APPEND = $(eval V += $$(file >>seen,RHS=[$$(V)])suffix)
READ := $(let V,local,$(APPEND)$(info inside=[$(V)/$(origin V)/$(flavor V)]))
all: ; @$(info outside=[$(V)/$(origin V)/$(flavor V)])cat seen
