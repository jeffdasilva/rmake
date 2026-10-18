export
A=value-A
B=value-B
C=value-C
D=value-D
E=value-E
F=value-F
G=guard
unexport $(NAMES)
all:
	@printf 'child-A=<%s>|%s\n' "$$A" "$${A+present}"
	@printf 'child-B=<%s>|%s\n' "$$B" "$${B+present}"
	@printf 'child-C=<%s>|%s\n' "$$C" "$${C+present}"
	@printf 'child-D=<%s>|%s\n' "$$D" "$${D+present}"
	@printf 'child-E=<%s>|%s\n' "$$E" "$${E+present}"
	@printf 'child-F=<%s>|%s\n' "$$F" "$${F+present}"
	@printf 'child-G=<%s>|%s\n' "$$G" "$${G+present}"
