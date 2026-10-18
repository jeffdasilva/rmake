K=old
define $(subst =,_,D=E) :=
$(K)
endef
define \$(subst =,_,F=G)
$(K)
endef
K=new
$(info simple=<$(value D_E)>|$(flavor D_E)|$(origin D_E))
$(info recursive=<$(value \F_G)>|$(flavor \F_G)|$(origin \F_G))
all:
	@true
