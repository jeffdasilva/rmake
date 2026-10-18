$(subst x,+,x)=P
$(subst x,?,x)=Q
$(subst x,:,x)=C
$(subst x,!,x)=B
$(subst x,=,x)=E
A$+=value
$(info case0=<$(value AP)>|$(flavor AP)|$(origin AP))
B$?=value
$(info case1=<$(value BQ)>|$(flavor BQ)|$(origin BQ))
C$:=value
$(info case2=<$(value CC)>|$(flavor CC)|$(origin CC))
D$!=value
$(info case3=<$(value DB)>|$(flavor DB)|$(origin DB))
E$==value
$(info case4=<$(value EE)>|$(flavor EE)|$(origin EE))
all:
	@true
