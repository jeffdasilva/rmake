.SECONDEXPANSION:
APPEND = $(eval 1 += $$(file >>seen,RHS=[$$(1)])suffix)$(info inside=[$(1)])
all: $$(call APPEND,input) ; @$(info outside=[$(1)/$(origin 1)/$(flavor 1)])cat seen
