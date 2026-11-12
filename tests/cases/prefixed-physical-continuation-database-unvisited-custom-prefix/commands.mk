.RECIPEPREFIX := >
all:;
unused:
> @printf '<%s>\n' "$(file >should-not-exist,WRONG)left \
>>right"
.RECIPEPREFIX := !
