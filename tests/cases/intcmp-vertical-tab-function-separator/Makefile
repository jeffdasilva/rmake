empty :=
space := $(empty) $(empty)
NAME := $(space)user suffix
INNER := $(space)inner suffix
0 = global-zero
1 = global-one
user = [$(0):$(1)]
inner = [$(0):$(1)]
outer = before[$(0):$(1)]$(call $(INNER),inner-value)after[$(0):$(1)]
all:
	@printf '[%s]\n' '$(intcmp1,1)' '$(intcmp1,2,L,E,G)' '$(callintcmpignored,1,2,L,E,G)' '$(call user suffix,plain)' '$(call $(NAME),expanded)' '$(callusersuffix,vertical)' '$(call outer trailing,outer-value)' '$(origin 0)/$(flavor 0)/$(value 0)/$(1)'
