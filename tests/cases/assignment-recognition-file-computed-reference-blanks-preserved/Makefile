$(subst _, ,A_B)=one
$(info case0=<$(value A B)>|$(flavor A B)|$(origin A B))
${subst _, ,${N}}=two
$(info case1=<$(value C D)>|$(flavor C D)|$(origin C D))
\$(subst _, ,E_F)=three
$(info case2=<$(value \E F)>|$(flavor \E F)|$(origin \E F))
'$(subst _, ,G_H)'=four
$(info case3=<$(value 'G H')>|$(flavor 'G H')|$(origin 'G H'))
pre$(M)post=five
$(info case4=<$(value preI Jpost)>|$(flavor preI Jpost)|$(origin preI Jpost))
 	$(subst _, ,K_L) 	=six
$(info case5=<$(value K L)>|$(flavor K L)|$(origin K L))
all:
	@true
