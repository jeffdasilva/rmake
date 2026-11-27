record = $(file >>seen,$(1)|$(MAKELEVEL)|$(origin MARK)|$(value MARK)|$(origin SIDE)|$(value SIDE)|$(value MAKEFLAGS)|$(MAKEFLAGS))
$(call record,CHILD_READ)

child: ; @$(call record,CHILD_RUN)cat seen
