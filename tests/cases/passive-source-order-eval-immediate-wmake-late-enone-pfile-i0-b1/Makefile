$(file >seen,anchor)
A=baseA
B=baseB
$(eval override GNUMAKEFLAGS = READY:=yes STATE:=$$$$(if\ $$$$(value\ GNUMAKEFLAGS),pending,cleared) A:::=$$$$(file\ >>seen,A)$$$$(info\ GNU_A)$$$$(MISSING_Aÿ))
$(eval override MAKEFLAGS = $$(if $$(filter environment,$$(origin READY)),$$(file >>seen,FLAGS)$$(info FLAGS A=[$$(A)]/$$(origin A)/$$(flavor A) STATE=[$$(STATE)] GNU=[$$(if $$(value GNUMAKEFLAGS),pending,cleared)] SEEN=[$$(file <seen)]) B:::=$$$$(file\ >>seen,B)$$$$(info\ GNU_B)$$$$(MISSING_Bÿ) --warn-undefined-variables))
all:;@$(eval unexport A B READY STATE)$(info RESULT B=[$(B)]/$(origin B)/$(flavor B) STATE=[$(STATE)] GNU=[$(GNUMAKEFLAGS)])$(file >>seen,DONE)$(info SEEN=[$(file <seen)]):
