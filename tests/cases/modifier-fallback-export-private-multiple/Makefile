X=kept
export private override private unexport export X
$(info X=<$(value X)>|$(origin X)|$(flavor X))
$(info export=<$(value export)>|$(origin export)|$(flavor export))
$(info unexport=<$(value unexport)>|$(origin unexport)|$(flavor unexport))
$(info override=<$(value override)>|$(origin override)|$(flavor override))
$(info private=<$(value private)>|$(origin private)|$(flavor private))
all:
	@printf "X=<%s>|%s export=<%s>|%s unexport=<%s>|%s override=<%s>|%s private=<%s>|%s\n" "$$X" "$${X+present}" "$$export" "$${export+present}" "$$unexport" "$${unexport+present}" "$$override" "$${override+present}" "$$private" "$${private+present}"
