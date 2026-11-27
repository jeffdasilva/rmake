$(file >>seen,CHILD_READ|$(MAKELEVEL)|$(origin EARLY)|$(value EARLY)|$(origin MARK)|$(value MARK)|$(origin SIDE)|$(value SIDE)|$(value MAKEFLAGS))
child: ; @$(file >>seen,CHILD_RUN|$(MAKELEVEL)|$(origin EARLY)|$(value EARLY)|$(origin MARK)|$(value MARK)|$(origin SIDE)|$(value SIDE)|$(value MAKEFLAGS)|$(MAKEFLAGS))cat seen
