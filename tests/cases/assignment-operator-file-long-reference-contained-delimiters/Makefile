$(subst =,_,A=B)=value
$(info case0=<$(value A_B)>|$(flavor A_B)|$(origin A_B))
${subst :,_,C:D}:=value
$(info case1=<$(value C_D)>|$(flavor C_D)|$(origin C_D))
$(subst !,_,E!F)?=value
$(info case2=<$(value E_F)>|$(flavor E_F)|$(origin E_F))
$(subst ?,_,${subst =,_,G=H?I})::=value
$(info case3=<$(value G_H_I)>|$(flavor G_H_I)|$(origin G_H_I))
pre$(subst =,_,J=K)post +=value
$(info case4=<$(value preJ_Kpost)>|$(flavor preJ_Kpost)|$(origin preJ_Kpost))
all:
	@true
