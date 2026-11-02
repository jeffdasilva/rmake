MAKEFLAGS = A?=$$(file\ >>seen,A)$$(MISSÿ_PRE)$$(eval\ MAKEFLAGS\ =\ --warn-undefined-variables)$$(MISSÿ_POST) B:=$$(file\ >>seen,B)$$(MISSÿ_B)
$(info RESULT A=[$(A)]/$(origin A)/$(flavor A) B=[$(B)]/$(origin B)/$(flavor B))$(file >>seen,DONE)$(info SEEN=[$(file <seen)])
